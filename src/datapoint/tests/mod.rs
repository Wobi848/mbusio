//! Tests der Datenpunkte: gemeinsame Helfer; Themen in werte, tabellen, ausgaenge

use super::*;
use crate::db::{Line, Meter};
use crate::mbus::master::{FrameLog, Master};
use crate::testutil::with_energy;
use std::sync::Arc;

const ENERGY: &str = "instantaneous|0|0|0|energy|kWh";

/// Wärmezähler 1 aus dem Simulator auslesen
fn heat_reading() -> MeterReading {
    let link = crate::link::open("SIMULATOR", 2400).unwrap();
    let mut m = Master::new(link, 2400, 60, Arc::new(FrameLog::default()));
    m.read_meter(&Target::Primary(1)).unwrap()
}

/// MQTT als Ganzes ein- oder ausschalten (ohne laufenden Broker: Nachrichten bleiben in der Warteschlange)
fn mqtt_on(db: &mut Db, on: bool) {
    let mut c = db.mqtt_config();
    c.enabled = on;
    c.host = "127.0.0.1".into();
    db.set_mqtt_config(&c).unwrap();
}

fn setup() -> (Db, i64, i64, MeterReading) {
    let mut db = Db::memory().unwrap();
    let line = db.active_line().unwrap().id;
    let m = db
        .add_meter(
            line,
            Meter {
                address: "1".into(),
                name: "Whg 1".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let r = heat_reading();
    db.record(line, &Target::Primary(1), Ok(&r), false).unwrap();
    (db, line, m.id, r)
}

mod ausgaenge;
mod tabellen;
mod werte;
