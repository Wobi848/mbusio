//! Linien-Dienst: besitzt die Verbindung einer Linie und arbeitet Aufträge nacheinander ab.
//!
//! Pro Linie spricht genau dieser Dienst mit dem Bus (Grundsatz „ein Bus, ein Besitzer“).
//! Alle anderen geben ihm Aufträge und warten auf das Ergebnis, statt „Bus belegt“ zu melden.
//! Aufträge aus der Bedienung haben Vorrang vor geplanten Auslesungen.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::i18n::{self, both, tr};
use crate::link;
use crate::mbus::master::{FrameLog, Master, MeterReading, Target};

/// Anschluss einer Linie
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineConf {
    pub connection: String,
    pub baud: u32,
    /// 0 = automatisch aus der Baudrate
    pub timeout_ms: u64,
}

impl LineConf {
    /// Anschluss, wie er bei der Linie gespeichert ist
    pub fn of(line: &crate::db::Line) -> Self {
        LineConf {
            connection: line.connection.clone(),
            baud: line.baud,
            timeout_ms: line.timeout_ms,
        }
    }
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

/// Offene Anschlüsse aller Linien: ein Anschluss gehört immer nur einer Linie. Gilt für die
/// Bedienung wie für den Ausleseplan, weil jeder Linien-Dienst hier belegt, bevor er öffnet.
#[derive(Default)]
pub struct Ports(Mutex<HashMap<String, (i64, String, u64)>>);

/// Belegter Anschluss; wird frei, sobald die Verbindung der Linie zu ist (Drop)
struct Claim {
    ports: Arc<Ports>,
    key: Option<String>,
    /// Nummer des Belegs: eine alte Verbindung, die spät schliesst, gibt den Beleg einer neuen
    /// Verbindung derselben Linie nicht frei
    id: u64,
}

impl Ports {
    /// Anschluss für die Linie belegen; besetzt: Fehler mit der Linie, die ihn hat
    fn claim(self: &Arc<Self>, port: &str, line_id: i64, label: &str) -> Result<Claim> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let key = link::port_key(port);
        if let Some(k) = &key {
            let mut held = self.0.lock().unwrap();
            match held.get(k) {
                Some((other, by, _)) if *other != line_id => {
                    return Err(anyhow!(tr(
                        &format!("{port} wird schon von Linie {by} benutzt"),
                        &format!("{port} is already used by line {by}"),
                    )))
                }
                _ => {
                    held.insert(k.clone(), (line_id, label.to_string(), id));
                }
            }
        }
        Ok(Claim {
            ports: self.clone(),
            key,
            id,
        })
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        if let Some(k) = &self.key {
            let mut held = self.ports.0.lock().unwrap();
            if held.get(k).is_some_and(|h| h.2 == self.id) {
                held.remove(k);
            }
        }
    }
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
    line_id: i64,
    /// Name der Linie für die Meldung „wird schon von Linie … benutzt“
    label: String,
    conf: LineConf,
    log: Arc<FrameLog>,
    ports: Arc<Ports>,
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

/// Liest einen Zähler der Liste, mit seiner eigenen Baudrate falls angegeben.
pub fn read_listed(m: &mut Master, meter: &crate::db::Meter) -> Result<(Target, MeterReading)> {
    let target = Target::parse(&meter.address)?;
    let r = m.at_baud(meter.baud, |m| m.read_meter(&target))?;
    Ok((target, r))
}

/// Ist der Fehler eine gestörte Verbindung (und kein Problem eines einzelnen Zählers)?
pub fn is_link_error(e: &anyhow::Error) -> bool {
    e.chain().any(|c| c.is::<std::io::Error>())
}

impl LineService {
    /// Startet den Dienst; verbunden wird beim ersten Auftrag.
    pub fn start(
        line_id: i64,
        label: String,
        conf: LineConf,
        log: Arc<FrameLog>,
        ports: Arc<Ports>,
    ) -> Self {
        let inner = Arc::new(Inner {
            line_id,
            label,
            conf,
            log,
            ports,
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
        f: impl FnOnce(&mut Master) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let lang = i18n::current();
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
    pub fn connect(&self) -> Result<()> {
        self.run(|_| Ok(()))
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

/// Anschluss belegen und öffnen; der Beleg gehört zur Verbindung und wird mit ihr frei
fn open(inner: &Inner) -> Result<(Master, Claim)> {
    let (conf, log) = (&inner.conf, &inner.log);
    let claim = inner
        .ports
        .claim(&conf.connection, inner.line_id, &inner.label)?;
    let link = link::open(&conf.connection, conf.baud)?;
    let master = Master::new(link, conf.baud, conf.timeout_ms, log.clone());
    log.push(
        "INFO",
        both(|| {
            format!(
                "{} {}, {} Bd, 8E1, Timeout {} ms",
                tr("Verbunden mit", "Connected to"),
                conf.connection,
                conf.baud,
                master.timeout.as_millis()
            )
        }),
    );
    Ok((master, claim))
}

const DEVICE_CHECK: Duration = Duration::from_secs(10);

/// Serieller Anschluss als Pfad (Linux, macOS), der verschwunden ist: USB-Wandler abgezogen.
/// Windows-COM-Ports und Gateways lassen sich so nicht prüfen, die merken es beim nächsten Auftrag.
fn device_gone(connection: &str) -> bool {
    connection.starts_with('/') && !std::path::Path::new(connection).exists()
}

fn run(inner: Arc<Inner>) {
    let mut master: Option<(Master, Claim)> = None;
    // nach einem gescheiterten Verbindungsversuch nicht sofort wieder (USB weg, Gateway aus)
    let mut retry_at: Option<Instant> = None;
    let mut backoff = Duration::from_secs(5);
    loop {
        // eigene Meldungen der Linie in der Sprache der Oberfläche; ein Auftrag aus der Bedienung
        // setzt seine Sprache selbst
        i18n::follow_ui();
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
                            .push("INFO", both(|| tr("Verbindung getrennt", "Disconnected")));
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
                // im Leerlauf alle 10 s prüfen, ob ein USB-Wandler noch da ist: so steht die
                // Linie gleich auf „getrennt“, nicht erst beim nächsten Termin (vielleicht morgen)
                q = inner.wake.wait_timeout(q, DEVICE_CHECK).unwrap().0;
                if master.is_some() && device_gone(&inner.conf.connection) {
                    master = None;
                    let msg = both(|| {
                        tr(
                            &format!(
                                "{}: nicht vorhanden – USB-Wandler abgezogen?",
                                inner.conf.connection
                            ),
                            &format!(
                                "{}: not present – USB converter unplugged?",
                                inner.conf.connection
                            ),
                        )
                    });
                    let mut s = inner.status.lock().unwrap();
                    s.connected = false;
                    s.connection = None;
                    s.error = Some(msg.get().to_string());
                    inner.log.push("INFO", msg);
                }
            }
        };
        // Verbindung bei Bedarf aufbauen; die Wartezeit nach Fehlversuchen gilt nur für
        // geplante Auslesungen, die Bedienung versucht es immer sofort (USB wieder eingesteckt)
        if master.is_none() && (manual || retry_at.is_none_or(|t| Instant::now() >= t)) {
            match open(&inner) {
                Ok((m, claim)) => {
                    let mut s = inner.status.lock().unwrap();
                    s.connected = true;
                    s.error = None;
                    s.connection = Some(ConnInfo {
                        port: inner.conf.connection.clone(),
                        baud: inner.conf.baud,
                        timeout_ms: m.timeout.as_millis() as u64,
                        timeout_auto: inner.conf.timeout_ms == 0,
                    });
                    master = Some((m, claim));
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
            Some((m, _)) => work(Ok(m)),
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
                both(|| {
                    tr(
                        "Verbindung gestört, wird neu aufgebaut",
                        "Connection broken, reconnecting",
                    )
                }),
            );
            let mut s = inner.status.lock().unwrap();
            s.connected = false;
            s.connection = None;
        }
    }
}

#[cfg(test)]
mod tests;
