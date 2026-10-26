//! Protokoll des Busses: Telegramme (TX/RX) und Meldungen, gezeigt in Werkzeuge und Bus-Konsole

use super::*;

/// Eintrag, wie ihn die Oberfläche bekommt: Text in der Sprache der Anfrage
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub seq: u64,
    pub time: String,
    pub dir: &'static str,
    pub text: String,
}

struct Stored {
    seq: u64,
    time: String,
    dir: &'static str,
    text: Text,
}

#[derive(Default)]
pub struct FrameLog {
    inner: Mutex<(u64, VecDeque<Stored>)>,
}

impl FrameLog {
    /// `text`: Telegramm oder Systemtext als `String`, Meldungen in beiden Sprachen mit `i18n::both`
    pub fn push(&self, dir: &'static str, text: impl Into<Text>) {
        let mut g = self.inner.lock().unwrap();
        g.0 += 1;
        let seq = g.0;
        g.1.push_back(Stored {
            seq,
            time: crate::clock::now().format("%H:%M:%S%.3f").to_string(),
            dir,
            text: text.into(),
        });
        while g.1.len() > 1000 {
            g.1.pop_front();
        }
    }

    pub fn since(&self, seq: u64) -> Vec<LogEntry> {
        self.inner
            .lock()
            .unwrap()
            .1
            .iter()
            .filter(|e| e.seq > seq)
            .map(|e| LogEntry {
                seq: e.seq,
                time: e.time.clone(),
                dir: e.dir,
                text: e.text.get().to_string(),
            })
            .collect()
    }

    pub fn clear(&self) {
        self.inner.lock().unwrap().1.clear();
    }
}
