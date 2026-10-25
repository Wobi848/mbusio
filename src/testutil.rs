//! Bausteine für Tests mehrerer Module (nur in Tests gebaut).

use crate::db::{Db, Line, Meter};
use crate::mbus::master::{FrameLog, Master, MeterReading, Target};

/// Kennung der Energie eines Wärmezählers
pub const ENERGY: &str = "instantaneous|0|0|0|energy|kWh";

/// Eine Linie im Dauerbetrieb mit einem ausgelesenen Wärmezähler (Simulator, Adresse 1) und der
/// Energie als Datenpunkt. Liefert Datenbank, Linie, Zähler und die Auslesung (für weitere).
pub fn db_with_energy() -> (Db, i64, i64, MeterReading) {
    let mut db = Db::memory().unwrap();
    let line = db.active_line().unwrap();
    db.update_line(
        line.id,
        Line {
            connection: "SIMULATOR".into(),
            ..line.clone()
        },
    )
    .unwrap();
    let mid = db
        .add_meter(
            line.id,
            Meter {
                address: "1".into(),
                ..Default::default()
            },
        )
        .unwrap()
        .id;
    db.set_line_plan(line.id, Some("hourly")).unwrap();
    db.set_operation(line.id, true).unwrap();
    let link = crate::link::open("SIMULATOR", 2400).unwrap();
    let mut m = Master::new(link, 2400, 60, std::sync::Arc::new(FrameLog::default()));
    let r = m.read_meter(&Target::Primary(1)).unwrap();
    db.record(line.id, &Target::Primary(1), Ok(&r), false)
        .unwrap();
    db.dp_select(mid, &[ENERGY.to_string()], &[], crate::i18n::Lang::De)
        .unwrap();
    // Datenpunkt hat jetzt einen Wert
    db.record(line.id, &Target::Primary(1), Ok(&r), false)
        .unwrap();
    (db, line.id, mid, r)
}

/// Dieselbe Auslesung mit einem anderen Energiewert
pub fn with_energy(r: &MeterReading, value: f64) -> MeterReading {
    let mut r = r.clone();
    for x in r
        .records
        .iter_mut()
        .filter(|x| crate::datapoint::key_of(x) == ENERGY)
    {
        x.value = serde_json::json!(value);
        x.value_text = value.to_string();
    }
    r
}
