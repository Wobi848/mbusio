//! Gemeinsamer Zustand der Weboberfläche: Linien-Dienste starten und stoppen, laufender Auftrag,
//! Datenbank, Ausgänge.

use super::*;

#[derive(Clone, Default, Serialize)]
pub struct JobStatus {
    pub kind: String,
    pub running: bool,
    pub done: u32,
    pub total: u32,
    pub message: String,
    pub hits: Vec<ScanHit>,
    pub error: Option<String>,
    /// Bereich des Scans (Adressen, Baudraten); verglichen wird nur mit gleichem Bereich
    #[serde(skip_serializing_if = "String::is_empty")]
    pub scope: String,
    /// Letzter vollständiger Scan gleicher Art und gleichen Bereichs, zum Vergleich: {time, hits}
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<serde_json::Value>,
    /// Abgebrochener Sekundärscan: hier geht „Weitersuchen“ weiter {mask, baud}
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume: Option<serde_json::Value>,
}

pub struct Shared {
    /// Linien-Dienste, je verbundener Linie einer; nur er spricht mit dem Bus dieser Linie
    pub(super) lines: Mutex<HashMap<i64, LineService>>,
    /// Offene Anschlüsse aller Linien (zwei Linien, ein Port: abgelehnt)
    pub(super) ports: Arc<Ports>,
    pub(super) job: Mutex<JobStatus>,
    pub(super) cancel: AtomicBool,
    pub(crate) log: Arc<FrameLog>,
    /// Projekte, Linien, Zähler, Auslesungen; Zählerliste und Scan gelten für die aktive Linie
    pub(crate) db: Mutex<Db>,
    /// Zeitplan der geplanten Auslesungen
    pub(crate) planner: Planner,
    /// MQTT-Ausgang: Zustand und Version der Einstellungen
    pub(crate) mqtt: crate::mqtt::MqttState,
    /// Modbus-Server: Zustand, Version der Einstellungen, Registerbild
    pub(crate) modbus: crate::modbus::ModbusState,
    /// Optionaler Zugriffsschlüssel für die API (bei LAN-Zugriff)
    pub(super) token: Option<String>,
    /// true beim Herunterfahren: offene Ströme (Node-RED) enden, sonst wartet der Server auf sie
    pub(crate) stop: tokio::sync::watch::Sender<bool>,
}

impl Shared {
    pub fn new(token: Option<String>, db: Db) -> Self {
        Self::with_planner(token, db, Planner::default())
    }

    pub fn with_planner(token: Option<String>, db: Db, planner: Planner) -> Self {
        if let Ok(Some(lang)) = db.meta("ui_lang") {
            crate::i18n::remember_ui(crate::i18n::Lang::parse(&lang));
        }
        Shared {
            lines: Mutex::default(),
            ports: Arc::default(),
            job: Mutex::default(),
            cancel: AtomicBool::default(),
            log: Arc::default(),
            db: Mutex::new(db),
            planner,
            mqtt: Default::default(),
            modbus: Default::default(),
            token,
            stop: tokio::sync::watch::channel(false).0,
        }
    }

    pub(super) fn line_id(&self) -> Result<i64, ApiError> {
        Ok(self.db.lock().unwrap().active_line()?.id)
    }

    /// Dienst der aktiven Linie; ohne Verbindung "Nicht verbunden"
    pub(super) fn service(&self) -> Result<LineService, ApiError> {
        let id = self.line_id()?;
        self.lines
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(not_connected)
    }

    /// Linien-Dienst arbeitet gerade oder hat Aufträge in der Warteschlange
    pub(super) fn line_busy(&self, line_id: i64) -> bool {
        let svc = self.lines.lock().unwrap().get(&line_id).cloned();
        svc.is_some_and(|s| {
            let st = s.status();
            st.busy || st.queued > 0
        })
    }

    /// Verbindung der aktiven Linie, falls offen
    pub(super) fn conn_info(&self) -> Option<ConnInfo> {
        self.service().ok()?.status().connection
    }

    /// Linie mit diesem Anschluss verbinden; ein vorhandener Dienst der Linie wird ersetzt.
    /// Klappt die Verbindung nicht, bleibt die Linie getrennt und der Fehler kommt zurück.
    pub fn connect_line(&self, line_id: i64, conf: LineConf) -> anyhow::Result<()> {
        self.planner.release(line_id);
        // läuft schon mit diesem Anschluss (z.B. wegen des Ausleseplans): weiter benutzen
        let running = self.lines.lock().unwrap().get(&line_id).cloned();
        if let Some(svc) = running.filter(|s| *s.conf() == conf) {
            return svc.connect();
        }
        self.stop_line_wait(line_id);
        let svc = self.start_service(line_id, conf);
        match svc.connect() {
            Ok(()) => {
                self.lines.lock().unwrap().insert(line_id, svc);
                Ok(())
            }
            Err(e) => {
                svc.stop();
                Err(e)
            }
        }
    }

    /// Linien-Dienst starten; belegt seinen Anschluss erst beim Verbinden (`Ports`)
    pub(super) fn start_service(&self, line_id: i64, conf: LineConf) -> LineService {
        let label = {
            let db = self.db.lock().unwrap();
            db.line(line_id)
                .and_then(|l| {
                    Ok((
                        l.name,
                        db.project(db.gateway(l.gateway_id)?.project_id)?.name,
                    ))
                })
                .map(|(l, p)| {
                    tr(
                        &format!("„{l}“ im Projekt „{p}“"),
                        &format!("“{l}” in project “{p}”"),
                    )
                })
                .unwrap_or_else(|_| line_id.to_string())
        };
        LineService::start(line_id, label, conf, self.log.clone(), self.ports.clone())
    }

    pub(super) fn stop_line(&self, line_id: i64) {
        if let Some(old) = self.lines.lock().unwrap().remove(&line_id) {
            old.stop();
        }
    }

    /// Wie `stop_line`, wartet aber, bis der Anschluss wieder frei ist (vor dem Neuöffnen)
    pub(super) fn stop_line_wait(&self, line_id: i64) {
        let old = self.lines.lock().unwrap().remove(&line_id);
        if let Some(old) = old {
            old.stop_wait(std::time::Duration::from_secs(15));
        }
    }

    /// Verbindungen trennen, z.B. weil eine andere Linie aktiv wird. Linien mit eingeschaltetem
    /// Ausleseplan bleiben verbunden.
    pub(super) fn stop_lines(&self) {
        let keep = self.planner.planned_lines();
        self.lines.lock().unwrap().retain(|id, svc| {
            let k = keep.contains(id) && !self.planner.is_held(*id);
            if !k {
                svc.stop();
            }
            k
        });
    }

    /// Dienst für den Zeitplan: läuft schon, oder wird mit dem Anschluss aus der Datenbank
    /// gestartet (verbindet beim ersten Auftrag). Anschluss geändert: neu starten.
    pub(crate) fn ensure_service(&self, line: &Line) -> Option<LineService> {
        let conf = LineConf::of(line);
        let running = self.lines.lock().unwrap().get(&line.id).cloned();
        match running {
            Some(svc) if *svc.conf() == conf => return Some(svc),
            // von Hand mit anderem Anschluss verbunden (aktive Linie): nicht dazwischenfunken
            Some(svc) if self.line_id().ok() == Some(line.id) && svc.status().connected => {
                return Some(svc);
            }
            Some(_) => self.stop_line_wait(line.id),
            None => {}
        }
        let svc = self.start_service(line.id, conf);
        self.lines.lock().unwrap().insert(line.id, svc.clone());
        Some(svc)
    }

    /// Alle Linien mit diesem Anschluss neu verbinden (z.B. weil die Simulation geändert wurde).
    /// Liefert die Anzahl.
    pub(crate) fn restart_connection(&self, connection: &str) -> usize {
        let ids: Vec<(i64, LineConf)> = self
            .lines
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, s)| s.conf().connection == connection)
            .map(|(id, s)| (*id, s.conf().clone()))
            .collect();
        let active = self.line_id().ok();
        for (id, conf) in &ids {
            self.stop_line_wait(*id);
            // die angesehene Linie gleich wieder verbinden; Linien mit Plan startet der Zeitplan
            if Some(*id) == active {
                if let Err(e) = self.connect_line(*id, conf.clone()) {
                    self.log.push("INFO", format!("{connection}: {e:#}"));
                }
            }
        }
        ids.len()
    }

    /// Dienste von Linien beenden, die weder einen Plan haben noch gerade angesehen werden
    pub(crate) fn stop_unused_services(&self) {
        let keep = self.planner.planned_lines();
        let active = self.line_id().ok();
        self.lines.lock().unwrap().retain(|id, svc| {
            let k = keep.contains(id) || Some(*id) == active;
            if !k {
                svc.stop();
            }
            k
        });
    }

    /// Auslesung speichern (Verlauf und Zählerliste der Linie); Speicherfehler nur ins Log
    pub(crate) fn record(&self, line: i64, target: &Target, result: Result<&MeterReading, String>) {
        let notes = {
            let mut db = self.db.lock().unwrap();
            if let Err(e) = db.record(line, target, result, false) {
                self.log.push("INFO", format!("DB: {e:#}"));
            }
            std::mem::take(&mut db.notes)
        };
        for n in notes {
            self.log.push("INFO", n);
        }
    }
}

pub type AppState = Arc<Shared>;
