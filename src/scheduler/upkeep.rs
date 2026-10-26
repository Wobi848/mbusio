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
