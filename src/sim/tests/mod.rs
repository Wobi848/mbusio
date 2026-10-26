//! Tests des Simulators über den echten Master: gemeinsame Helfer; Themen in scannen, auslesen,
//! einstellen, vorlagen

use super::*;
use crate::mbus::master::{Bauds, FrameLog, Master, MeterReading, Progress, Target};
use std::sync::Arc;

/// Status als &'static str für die Vergleiche in den Tests
fn stat(s: &str) -> &'static str {
    match s {
        "ok" => "ok",
        "collision" => "collision",
        _ => "no_data",
    }
}

fn master() -> Master {
    Master::new(
        Box::new(Simulator::new(9600)),
        9600,
        50,
        Arc::new(FrameLog::default()),
    )
}

fn master_for(key: &str, baud: u32) -> Master {
    Master::new(
        Box::new(super::open(key, baud).unwrap()),
        baud,
        50,
        Arc::new(FrameLog::default()),
    )
}

mod auslesen;
mod einstellen;
mod scannen;
mod vorlagen;
