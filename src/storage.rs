//! Schutz vor vollem Speicher (Konzept Dauerbetrieb, 4.5): freien Platz beim Datenordner
//! überwachen. Wenig frei: Warnung. Fast nichts frei: keine neuen Auslesungen mehr speichern,
//! damit die Datenbank intakt und das Gerät bedienbar bleibt.

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    #[default]
    Ok,
    /// unter 20 % frei: Warnung
    Low,
    /// unter 5 % frei: Auslesungen werden nicht mehr gespeichert
    Full,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DiskState {
    pub level: Level,
    /// freier Anteil in Prozent
    pub free_pct: f64,
    pub free_bytes: u64,
    /// Grösse der Datenbank
    pub db_bytes: u64,
}

const GB: u64 = 1024 * 1024 * 1024;

/// Prozent nach Konzept; auf grossen Platten zählt zusätzlich der absolute Platz, sonst würde
/// ein Laptop mit 2 TB schon bei 400 GB frei warnen
pub fn level_of(free: u64, total: u64) -> Level {
    let pct = if total == 0 {
        100.0
    } else {
        free as f64 * 100.0 / total as f64
    };
    if (pct < 5.0 && free < GB) || free < 200 * 1024 * 1024 {
        Level::Full
    } else if (pct < 20.0 && free < 10 * GB) || free < GB {
        Level::Low
    } else {
        Level::Ok
    }
}

/// Freien Platz beim Ordner der Datenbank messen; unbekannt (Fehler) gilt als in Ordnung
pub fn check(dir: &Path, db_bytes: u64) -> DiskState {
    match (fs4::available_space(dir), fs4::total_space(dir)) {
        (Ok(free), Ok(total)) => DiskState {
            level: level_of(free, total),
            free_pct: if total == 0 {
                100.0
            } else {
                free as f64 * 100.0 / total as f64
            },
            free_bytes: free,
            db_bytes,
        },
        _ => DiskState {
            db_bytes,
            free_pct: 100.0,
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels() {
        // SD-Karte 32 GB
        let sd = 32 * GB;
        assert_eq!(level_of(sd / 2, sd), Level::Ok);
        assert_eq!(level_of(sd / 10, sd), Level::Low);
        assert_eq!(level_of(sd / 50, sd), Level::Full);
        // grosse Platte: 15 % frei sind noch 300 GB, keine Warnung
        let big = 2000 * GB;
        assert_eq!(level_of(big * 15 / 100, big), Level::Ok);
        assert_eq!(level_of(5 * GB, big), Level::Low);
        assert_eq!(level_of(100 * 1024 * 1024, big), Level::Full);
    }

    #[test]
    fn measures_this_disk() {
        let s = check(&std::env::temp_dir(), 0);
        assert!(s.free_bytes > 0 && s.free_pct > 0.0);
    }
}
