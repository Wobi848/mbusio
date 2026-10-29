//! Pflege nebenher: freier Speicher, Verlauf ausdünnen, Uhrzeit und Sprünge der Uhr.

use super::*;

/// Freien Speicher prüfen; bei fast vollem Speicher keine Auslesungen mehr speichern
pub(super) fn check_disk(st: &AppState) {
    let (dir, size) = {
        let db = st.db.lock().unwrap();
        (
            db.path().and_then(|p| p.parent()).map(|p| p.to_path_buf()),
            db.size(),
        )
    };
    let Some(dir) = dir else { return };
    let new = storage::check(&dir, size);
    let old = std::mem::replace(&mut *st.planner.disk.lock().unwrap(), new.clone());
    st.db.lock().unwrap().readings_paused = new.level == Level::Full;
    if new.level != old.level {
        let msg = match new.level {
            Level::Ok => tr("Speicher wieder in Ordnung", "Storage OK again"),
            Level::Low => tr("Speicher wird knapp", "Storage running low"),
            Level::Full => tr(
                "Speicher fast voll: neue Auslesungen werden nicht mehr gespeichert",
                "Storage almost full: new readings are no longer stored",
            ),
        };
        note(
            st,
            both(|| format!("{msg} ({:.0} % {})", new.free_pct, tr("frei", "free"))),
        );
    }
}

/// Verlauf ausdünnen, wo eingeschaltet (höchstens einmal pro Stunde)
pub(super) fn thin(st: &AppState, now: NaiveDateTime) -> anyhow::Result<()> {
    {
        let mut last = st.planner.last_thin.lock().unwrap();
        if last.is_some_and(|t| (now - t).num_minutes().abs() < 60) {
            return Ok(());
        }
        *last = Some(now);
    }
    let dropped = st.db.lock().unwrap().mqtt_trim()?;
    if dropped > 0 {
        note(
            st,
            both(|| {
                format!(
                    "MQTT: {dropped} {}",
                    tr(
                        "alte Nachrichten verworfen (Broker zu lange nicht erreichbar)",
                        "old messages dropped (broker unreachable for too long)"
                    )
                )
            }),
        );
    }
    // Protokoll der Weitergabe: 30 Tage, Grenze an Zeilen
    st.db.lock().unwrap().out_log_trim()?;
    protect_storage(st, now)?;
    let n = st.db.lock().unwrap().thin_readings(now)?;
    if n > 0 {
        note(
            st,
            both(|| {
                format!(
                    "{}: {n} {}",
                    tr("Verlauf ausgedünnt", "History thinned"),
                    tr("Auslesungen gelöscht", "readings deleted")
                )
            }),
        );
    }
    Ok(())
}

/// Fällige Abrechnungen als Datei ablegen (Konzept Export 3.5) und an die Empfänger senden
/// (Teil C), alle 15 Minuten. Gesendet wird ohne die Datenbank zu sperren.
pub(super) fn export_billing(st: &AppState, now: NaiveDateTime) -> anyhow::Result<()> {
    {
        let mut last = st.planner.last_export.lock().unwrap();
        if last.is_some_and(|t| (now - t).num_minutes().abs() < 15) {
            return Ok(());
        }
        *last = Some(now);
    }
    let db = st.db.lock().unwrap();
    let Some(dir) = db.path().and_then(|p| p.parent()).map(|p| p.to_path_buf()) else {
        return Ok(());
    };
    let mut notes = db.billing_auto(&dir, now)?;
    let (outgoing, problems) = db.billing_mail_pending(&dir, now)?;
    drop(db);
    notes.extend(problems);
    for o in outgoing {
        let result = crate::mail::send(&o.config, &o.message);
        let mut db = st.db.lock().unwrap();
        db.mail_record(&o.message, &result)?;
        notes.extend(db.billing_mail_done(o.project_id, &o.name, result, now)?);
    }
    for msg in notes {
        note(st, msg);
    }
    Ok(())
}

/// Fällige automatische Sicherung (Konzept Sicherung §5) in einem eigenen Thread, damit der
/// Zeitplan nicht auf das Netz wartet; läuft nie zwei gleichzeitig
pub(super) fn backup_auto(st: &AppState, now: NaiveDateTime) {
    let (config, due) = {
        let db = st.db.lock().unwrap();
        let config = db.backup_config();
        let due = config.enabled && db.backup_state().due(&config.time, now);
        (config, due)
    };
    if !due {
        return;
    }
    {
        let mut running = st.planner.backup_running.lock().unwrap();
        if *running {
            return;
        }
        *running = true;
    }
    let st = st.clone();
    std::thread::spawn(move || {
        let result = run_backup(&st, &config);
        if let Err(e) = &result {
            note(&st, both(|| format!("{}: {e:#}", tr("Sicherung", "Backup"))));
        }
        if let Err(e) = st
            .db
            .lock()
            .unwrap()
            .backup_record(&config.target_label(), &result, now)
        {
            note(&st, format!("Sicherung: {e:#}"));
        }
        *st.planner.backup_running.lock().unwrap() = false;
    });
}

fn run_backup(st: &AppState, config: &crate::backup::auto::BackupConfig) -> anyhow::Result<u64> {
    let (device, bytes) = {
        let db = st.db.lock().unwrap();
        let device = crate::backup::safe_device_name(&db.mail_config().device_name());
        (device, crate::backup::auto::build(&db)?)
    };
    crate::backup::auto::store(config, &device, &bytes)
}

/// Speicher knapp: selbst ausdünnen und verkleinern (Konzept Dauerbetrieb 4.5), danach neu messen
fn protect_storage(st: &AppState, now: NaiveDateTime) -> anyhow::Result<()> {
    let disk = st.planner.disk();
    if disk.level == Level::Ok {
        return Ok(());
    }
    let done = {
        let mut db = st.db.lock().unwrap();
        let s = db.storage_settings();
        db.protect_storage(now, &s, disk.free_bytes)?
    };
    let Some(p) = done else { return Ok(()) };
    let day = (now - chrono::Duration::days(p.days))
        .format("%d.%m.%Y")
        .to_string();
    let mb = p.freed / 1_000_000;
    note(
        st,
        both(|| {
            format!(
                "{}: {} {day} {} ({} {}, {mb} MB {})",
                tr("Speicher knapp", "Storage low"),
                tr("Auslesungen vor dem", "readings before"),
                tr("auf eine je Tag ausgedünnt", "thinned to one per day"),
                p.deleted,
                tr("gelöscht", "deleted"),
                tr("frei geworden", "freed")
            )
        }),
    );
    check_disk(st);
    Ok(())
}

/// Zeitquelle prüfen und Sprünge der Uhr erkennen
pub(super) fn check_clock(st: &AppState, now: NaiveDateTime) {
    let p = &st.planner;
    let mut last = p.last_clock_check.lock().unwrap();
    if last.is_none_or(|t| t.elapsed() >= CLOCK_CHECK) {
        *last = Some(Instant::now());
        if let Some(msg) = clock::check_system() {
            note(st, msg);
        }
        drop(last);
        check_disk(st);
        // geänderte Qualität (veraltet, gestört) an den Broker melden
        if let Err(e) = st.db.lock().unwrap().outputs_refresh() {
            note(st, format!("MQTT: {e:#}"));
        }
    } else {
        drop(last);
    }
    let mut tick = p.last_tick.lock().unwrap();
    if let Some((wall, mono)) = *tick {
        let elapsed = mono.elapsed().mul_f64(clock::scale());
        let expected = wall + chrono::Duration::from_std(elapsed).unwrap_or_default();
        let jump = (now - expected).num_seconds();
        if jump.abs() > 60 {
            note(
                st,
                both(|| {
                    format!(
                        "{} {:+} min ({} → {}); {}",
                        tr("Uhr gesprungen um", "Clock jumped by"),
                        jump / 60,
                        expected.format("%d.%m. %H:%M"),
                        now.format("%d.%m. %H:%M"),
                        tr("Termine richten sich neu aus", "terms realign")
                    )
                }),
            );
        }
    }
    *tick = Some((now, Instant::now()));
}
