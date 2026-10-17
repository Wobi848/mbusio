//! Linien-Dienst: besitzt die Verbindung einer Linie und arbeitet Aufträge nacheinander ab.
//!
//! Pro Linie spricht genau dieser Dienst mit dem Bus (Grundsatz „ein Bus, ein Besitzer“).
//! Alle anderen geben ihm Aufträge und warten auf das Ergebnis, statt „Bus belegt“ zu melden.
//! Aufträge aus der Bedienung haben Vorrang vor geplanten Auslesungen.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::i18n::{self, tr, Lang};
use crate::link;
use crate::mbus::master::{FrameLog, Master};

/// Anschluss einer Linie
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineConf {
    pub connection: String,
    pub baud: u32,
    /// 0 = automatisch aus der Baudrate
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ConnInfo {
    pub port: String,
    pub baud: u32,
    pub timeout_ms: u64,
    /// Timeout wird aus der Baudrate berechnet (für die Dauer-Schätzung im Browser)
    pub timeout_auto: bool,
}

/// Was die Oberfläche über eine Linie wissen muss
#[derive(Debug, Clone, Default, Serialize)]
pub struct LineStatus {
    /// Verbindung offen
    pub connected: bool,
    pub connection: Option<ConnInfo>,
    /// letzter Verbindungsfehler, solange keine Verbindung besteht
    pub error: Option<String>,
    /// gerade ein Auftrag in Arbeit
    pub busy: bool,
    /// Aufträge in der Warteschlange (ohne den laufenden)
    pub queued: usize,
}

/// Ein Auftrag bekommt den Master oder den Grund, warum keine Verbindung besteht.
/// Rückgabe `false`: die Verbindung ist gestört und wird vor dem nächsten Auftrag neu aufgebaut.
pub type Work = Box<dyn FnOnce(Result<&mut Master, String>) -> bool + Send>;

#[derive(Default)]
struct Queues {
    manual: VecDeque<Work>,
    planned: VecDeque<Work>,
}

struct Inner {
    conf: LineConf,
    log: Arc<FrameLog>,
    queues: Mutex<Queues>,
    wake: Condvar,
    stop: AtomicBool,
    status: Mutex<LineStatus>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

/// Griff auf den Dienst einer Linie; billig zu klonen.
#[derive(Clone)]
pub struct LineService {
    inner: Arc<Inner>,
}

/// Ist der Fehler eine gestörte Verbindung (und kein Problem eines einzelnen Zählers)?
pub fn is_link_error(e: &anyhow::Error) -> bool {
    e.chain().any(|c| c.is::<std::io::Error>())
}

impl LineService {
    /// Startet den Dienst; verbunden wird beim ersten Auftrag.
    pub fn start(line_id: i64, conf: LineConf, log: Arc<FrameLog>) -> Self {
        let inner = Arc::new(Inner {
            conf,
            log,
            queues: Mutex::default(),
            wake: Condvar::new(),
            stop: AtomicBool::new(false),
            status: Mutex::default(),
            thread: Mutex::default(),
        });
        let worker = inner.clone();
        let handle = std::thread::Builder::new()
            .name(format!("line-{line_id}"))
            .spawn(move || run(worker))
            .expect("thread");
        *inner.thread.lock().unwrap() = Some(handle);
        LineService { inner }
    }

    pub fn conf(&self) -> &LineConf {
        &self.inner.conf
    }

    pub fn status(&self) -> LineStatus {
        let mut s = self.inner.status.lock().unwrap().clone();
        let q = self.inner.queues.lock().unwrap();
        s.queued = q.manual.len() + q.planned.len();
        s
    }

    /// Auftrag aus der Bedienung (Vorrang)
    pub fn submit(&self, work: Work) {
        self.inner.queues.lock().unwrap().manual.push_back(work);
        self.inner.wake.notify_one();
    }

    /// Geplante Auslesung: kommt nach allen Aufträgen aus der Bedienung
    pub fn submit_planned(&self, work: Work) {
        self.inner.queues.lock().unwrap().planned.push_back(work);
        self.inner.wake.notify_one();
    }

    /// Auftrag aus der Bedienung ausführen und auf das Ergebnis warten (blockierend).
    pub fn run<T: Send + 'static>(
        &self,
        lang: Lang,
        f: impl FnOnce(&mut Master) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (tx, rx) = mpsc::channel();
        self.submit(Box::new(move |m| {
            i18n::set(lang);
            let (result, ok) = match m {
                Ok(m) => {
                    let r = f(m);
                    let ok = !matches!(&r, Err(e) if is_link_error(e));
                    (r, ok)
                }
                Err(e) => (Err(anyhow!(e)), true),
            };
            let _ = tx.send(result);
            ok
        }));
        rx.recv()
            .map_err(|_| anyhow!(tr("Linie wurde beendet", "Line was stopped")))?
    }

    /// Verbindung aufbauen (bzw. prüfen), ohne etwas auf den Bus zu senden
    pub fn connect(&self, lang: Lang) -> Result<()> {
        self.run(lang, |_| Ok(()))
    }

    /// Dienst beenden; laufender Auftrag wird zu Ende geführt, wartende bekommen einen Fehler.
    pub fn stop(&self) {
        self.inner.stop.store(true, Ordering::SeqCst);
        self.inner.wake.notify_all();
    }

    /// Beenden und warten, bis die Verbindung zu ist (höchstens `max`), z.B. bevor derselbe
    /// Anschluss neu geöffnet wird. Liefert false, wenn der laufende Auftrag länger dauert.
    pub fn stop_wait(&self, max: Duration) -> bool {
        self.stop();
        let Some(handle) = self.inner.thread.lock().unwrap().take() else {
            return true;
        };
        let until = Instant::now() + max;
        while !handle.is_finished() {
            if Instant::now() >= until {
                // weiterlaufen lassen; er beendet sich nach dem laufenden Auftrag selbst
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = handle.join();
        true
    }
}

fn open(conf: &LineConf, log: &Arc<FrameLog>) -> Result<Master> {
    let link = link::open(&conf.connection, conf.baud)?;
    let master = Master::new(link, conf.baud, conf.timeout_ms, log.clone());
    log.push(
        "INFO",
        format!(
            "{} {}, {} Bd, 8E1, Timeout {} ms",
            tr("Verbunden mit", "Connected to"),
            conf.connection,
            conf.baud,
            master.timeout.as_millis()
        ),
    );
    Ok(master)
}

fn run(inner: Arc<Inner>) {
    let mut master: Option<Master> = None;
    // nach einem gescheiterten Verbindungsversuch nicht sofort wieder (USB weg, Gateway aus)
    let mut retry_at: Option<Instant> = None;
    let mut backoff = Duration::from_secs(5);
    loop {
        let (work, manual) = {
            let mut q = inner.queues.lock().unwrap();
            loop {
                if inner.stop.load(Ordering::SeqCst) {
                    // wartende Aufträge sauber beenden, damit niemand ewig wartet
                    let rest: Vec<Work> = {
                        let q = &mut *q;
                        q.manual.drain(..).chain(q.planned.drain(..)).collect()
                    };
                    drop(q);
                    for w in rest {
                        w(Err(tr("Verbindung getrennt", "Disconnected")));
                    }
                    if master.take().is_some() {
                        inner
                            .log
                            .push("INFO", tr("Verbindung getrennt", "Disconnected"));
                    }
                    *inner.status.lock().unwrap() = LineStatus::default();
                    return;
                }
                if let Some(w) = q.manual.pop_front() {
                    break (w, true);
                }
                if let Some(w) = q.planned.pop_front() {
                    break (w, false);
                }
                q = inner.wake.wait(q).unwrap();
            }
        };
        // Verbindung bei Bedarf aufbauen; die Wartezeit nach Fehlversuchen gilt nur für
        // geplante Auslesungen, die Bedienung versucht es immer sofort (USB wieder eingesteckt)
        if master.is_none() && (manual || retry_at.is_none_or(|t| Instant::now() >= t)) {
            match open(&inner.conf, &inner.log) {
                Ok(m) => {
                    let mut s = inner.status.lock().unwrap();
                    s.connected = true;
                    s.error = None;
                    s.connection = Some(ConnInfo {
                        port: inner.conf.connection.clone(),
                        baud: inner.conf.baud,
                        timeout_ms: m.timeout.as_millis() as u64,
                        timeout_auto: inner.conf.timeout_ms == 0,
                    });
                    master = Some(m);
                    retry_at = None;
                    backoff = Duration::from_secs(5);
                }
                Err(e) => {
                    let msg = format!("{e:#}");
                    let mut s = inner.status.lock().unwrap();
                    s.connected = false;
                    s.connection = None;
                    s.error = Some(msg);
                    retry_at = Some(Instant::now() + backoff);
                    backoff = (backoff * 2).min(Duration::from_secs(60));
                }
            }
        }
        inner.status.lock().unwrap().busy = true;
        let ok = match master.as_mut() {
            Some(m) => work(Ok(m)),
            None => {
                let why = inner
                    .status
                    .lock()
                    .unwrap()
                    .error
                    .clone()
                    .unwrap_or_else(|| tr("Keine Verbindung", "Not connected"));
                work(Err(why))
            }
        };
        inner.status.lock().unwrap().busy = false;
        if !ok && master.take().is_some() {
            // gestörte Verbindung: beim nächsten Auftrag neu aufbauen
            inner.log.push(
                "INFO",
                tr(
                    "Verbindung gestört, wird neu aufgebaut",
                    "Connection broken, reconnecting",
                ),
            );
            let mut s = inner.status.lock().unwrap();
            s.connected = false;
            s.connection = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sim() -> LineService {
        LineService::start(
            1,
            LineConf {
                connection: "SIMULATOR".into(),
                baud: 9600,
                timeout_ms: 50,
            },
            Arc::new(FrameLog::default()),
        )
    }

    #[test]
    fn runs_jobs_in_order_with_manual_first() {
        let s = sim();
        // ersten Auftrag blockieren, bis die anderen in der Schlange stehen
        let (go_tx, go_rx) = mpsc::channel::<()>();
        let order = Arc::new(Mutex::new(Vec::new()));
        s.submit(Box::new(move |_| {
            go_rx.recv().unwrap();
            true
        }));
        for (planned, tag) in [(true, "p1"), (true, "p2"), (false, "m1")] {
            let o = order.clone();
            let w: Work = Box::new(move |_| {
                o.lock().unwrap().push(tag);
                true
            });
            if planned {
                s.submit_planned(w);
            } else {
                s.submit(w);
            }
        }
        // ein geplanter Auftrag ganz am Schluss meldet, wenn alles durch ist
        let (done_tx, done_rx) = mpsc::channel::<()>();
        s.submit_planned(Box::new(move |_| {
            done_tx.send(()).unwrap();
            true
        }));
        go_tx.send(()).unwrap();
        done_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(*order.lock().unwrap(), vec!["m1", "p1", "p2"]);
        s.stop();
    }

    #[test]
    fn reads_and_reports_status() {
        let s = sim();
        let r = s
            .run(Lang::De, |m| {
                m.read_meter(&crate::mbus::master::Target::Primary(1))
            })
            .unwrap();
        assert_eq!(r.header.unwrap().id, "12345678");
        // "arbeitet" geht erst kurz nach dem Ergebnis aus
        let until = Instant::now() + Duration::from_secs(5);
        while s.status().busy && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        let st = s.status();
        assert!(st.connected && !st.busy);
        s.stop();
    }

    #[test]
    fn unreachable_port_reports_error_and_retries_later() {
        let s = LineService::start(
            2,
            LineConf {
                connection: "/dev/does-not-exist".into(),
                baud: 2400,
                timeout_ms: 0,
            },
            Arc::new(FrameLog::default()),
        );
        assert!(s.connect(Lang::De).is_err());
        let st = s.status();
        assert!(!st.connected && st.error.is_some());
        // Bedienung versucht es sofort wieder, nicht erst nach der Wartezeit
        assert!(s.connect(Lang::De).is_err());
        s.stop();
    }
}
