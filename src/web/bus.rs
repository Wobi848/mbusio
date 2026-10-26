//! Verbindung (Ports, Status, Verbinden, Trennen) und laufender Auftrag (starten, Zustand, Abbruch)

use super::*;

pub(super) async fn ports() -> Json<Vec<link::PortInfo>> {
    Json(
        tokio::task::spawn_blocking(link::list_ports)
            .await
            .unwrap_or_default(),
    )
}

pub(super) async fn status(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    let line = st.service().ok().map(|s| s.status());
    let plans = st.planner.runs();
    let conn = line.as_ref().and_then(|l| l.connection.clone());
    let running = st.job.lock().unwrap().running;
    let db = st.db.lock().unwrap();
    let active = db.active()?;
    Ok(Json(json!({
        "connected": conn.is_some(),
        "connection": conn,
        "job_running": running,
        "readings": db.reading_count(active.line.id)?,
        "version": env!("CARGO_PKG_VERSION"),
        "token_required": st.token.is_some(),
        "meters_file": db.path().map(|p| p.display().to_string()),
        "active": active,
        // Zustand des Linien-Dienstes: arbeitet, Warteschlange, Verbindungsfehler
        "line": line,
        // laufende geplante Auslesungen je Linie
        "plans": plans,
        "plan_active": st.planner.any_planned(),
        "held": st.planner.is_held(active.line.id),
        "time": clock::state(),
        // Vorschau aus dem Plan, wie er jetzt ist (der Platz wird jede Minute gemessen); lässt
        // sie sich nicht berechnen, steht keine da
        "disk": st.planner.disk().with_growth(db.growth_per_day().unwrap_or(0)),
        "mqtt": mqtt_status(&st, &db),
        "modbus": modbus_status(&st),
        // alle Ausgänge: Schlüssel, Name, eingeschaltet
        "outputs": db.outputs_info(),
        "now": clock::stamp(),
        // läuft weiter ohne Anmeldung (Windows-Dienst, systemd); sonst Hinweis im Dauerbetrieb
        "background": crate::service::background(),
        "os": std::env::consts::OS,
    })))
}

#[derive(Deserialize)]
pub(super) struct ConnectReq {
    port: String,
    baud: u32,
    #[serde(default)]
    timeout_ms: u64,
}

pub(super) async fn connect(
    State(st): State<AppState>,
    Json(req): Json<ConnectReq>,
) -> ApiResult<serde_json::Value> {
    if st.job.lock().unwrap().running {
        return Err(busy());
    }
    let line = st.line_id()?;
    let conf = LineConf {
        connection: req.port.clone(),
        baud: req.baud,
        timeout_ms: req.timeout_ms,
    };
    let s = st.clone();
    blocking(move || s.connect_line(line, conf))
        .await?
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
    // erfolgreiche Verbindung gehört zur aktiven Linie: beim nächsten Mal wieder so verbinden
    st.db
        .lock()
        .unwrap()
        .set_line_connection(line, &req.port, req.baud, req.timeout_ms)?;
    Ok(Json(json!({ "ok": true, "connection": st.conn_info() })))
}

pub(super) async fn disconnect(State(st): State<AppState>) -> ApiResult<serde_json::Value> {
    ensure_idle(&st)?;
    let line = st.line_id()?;
    // von Hand getrennt (z.B. um den Anschluss für ein anderes Programm frei zu machen):
    // der Ausleseplan verbindet nicht von selbst wieder, bis man wieder verbindet
    st.planner.hold(line);
    let planned = st.planner.is_planned(line);
    st.stop_line(line);
    Ok(Json(json!({ "ok": true, "plan_paused": planned })))
}

pub(super) fn start_job<F>(
    st: &AppState,
    kind: &str,
    scope: String,
    hits: Vec<ScanHit>,
    work: F,
) -> ApiResult<JobStatus>
where
    F: FnOnce(&mut Master, &mut dyn FnMut(Progress) -> bool) -> anyhow::Result<()> + Send + 'static,
{
    let svc = st.service()?;
    {
        let mut job = st.job.lock().unwrap();
        if job.running {
            return Err(busy());
        }
        *job = JobStatus {
            kind: kind.into(),
            scope,
            hits,
            running: true,
            message: String::new(),
            ..Default::default()
        };
    }
    st.cancel.store(false, Ordering::SeqCst);
    st.log
        .push("INFO", format!("{} {kind}", tr("Starte", "Starting")));
    // Ergebnis gehört zur Linie, die beim Start aktiv war
    let line = st.line_id()?;
    let lang = i18n::current();
    let s = st.clone();
    // als Auftrag an den Linien-Dienst; der Fortschritt läuft über job
    svc.submit(Box::new(move |m| {
        i18n::set(lang);
        let result = match m {
            Err(e) => Err(anyhow::anyhow!(e)),
            Ok(master) => {
                let mut on_progress = |p: Progress| {
                    let mut job = s.job.lock().unwrap();
                    match p {
                        Progress::Step {
                            done,
                            total,
                            message,
                        } => {
                            job.done = done;
                            job.total = total;
                            job.message = message;
                        }
                        Progress::Hit(h) => job.hits.push(h),
                        Progress::Stopped { mask, baud } => {
                            job.resume = Some(json!({ "mask": mask, "baud": baud }))
                        }
                    }
                    !s.cancel.load(Ordering::SeqCst)
                };
                work(master, &mut on_progress)
            }
        };
        let link_ok = !matches!(&result, Err(e) if is_link_error(e));
        let mut job = s.job.lock().unwrap();
        job.running = false;
        if let Err(e) = result {
            job.error = Some(format!("{e:#}"));
        } else if s.cancel.load(Ordering::SeqCst) {
            job.message = tr("Abgebrochen", "Cancelled");
        }
        s.log.push(
            "INFO",
            both(|| match job.kind.as_str() {
                "read_all" => format!(
                    "{}: {}/{}",
                    tr("Alle auslesen beendet", "Read all finished"),
                    job.done,
                    job.total
                ),
                kind => format!(
                    "{} {}: {}",
                    if kind == "secondary" {
                        tr("Sekundärscan", "Secondary scan")
                    } else {
                        tr("Primärscan", "Primary scan")
                    },
                    tr("beendet, gefunden", "finished, found"),
                    job.hits.len()
                ),
            }),
        );
        if job.kind != "read_all" {
            // nur ein vollständiger Scan wird verglichen und für den nächsten Vergleich gemerkt
            let complete = job.error.is_none() && !s.cancel.load(Ordering::SeqCst);
            let mut db = s.db.lock().unwrap();
            if complete {
                job.previous = db.previous_scan(line, &job.kind, &job.scope).ok().flatten();
            }
            let mut value = serde_json::to_value(&*job).unwrap_or_default();
            // „PA ändern“ führt den gemerkten Scan nur nach, wenn dies ein vollständiger Scan war
            value["complete"] = json!(complete);
            if let Err(e) = db.set_last_scan(line, &value, complete) {
                s.log.push("INFO", format!("DB: {e:#}"));
            }
        }
        link_ok
    }));
    Ok(Json(st.job.lock().unwrap().clone()))
}

pub(super) async fn job(State(st): State<AppState>) -> Json<JobStatus> {
    Json(st.job.lock().unwrap().clone())
}

pub(super) async fn cancel(State(st): State<AppState>) -> Json<serde_json::Value> {
    st.cancel.store(true, Ordering::SeqCst);
    Json(json!({ "ok": true }))
}
