//! Simulator: virtueller M-Bus mit mehreren Slaves, zum Testen ohne Hardware.
//!
//! Belegung:
//! - Adr. 1: Wärmezähler, ID 12345678, Antwort in zwei Telegrammen
//! - Adr. 2: Elektrizitätszähler, ID 87654321
//! - Adr. 3: Wasserzähler, ID 30000001
//! - Adr. 0: zwei unkonfigurierte Wasserzähler (ID 12345699 und 55500011),
//!   kollidieren bei Primäradressierung, nur per Sekundäradresse erreichbar
//! - Adr. 4: Zähler mit falscher Prüfsumme im Datentelegramm
//! - Adr. 5: Zähler, dessen Datentelegramm abgeschnitten ist
//! - Adr. 6: Wasserzähler, ID 60000006, läuft mit 9600 Bd (bzw. 2400 Bd, wenn mit 9600 verbunden)
//! - Adr. 7: alter Wärmezähler, ID 70000007, feste Datenstruktur (CI 73)
//!
//! Typische Fehler aus der Praxis:
//! - Adr. 9: uralter Wasserzähler, ID 90000009, läuft mit 300 Bd
//! - Adr. 10: zwei Wärmezähler (ID 10000010 und 10000011) mit derselben Primäradresse
//! - Adr. 11: Zähler meldet "beschäftigt" (Fehlertelegramm CI 70), ID 11000011
//! - Adr. 12: Zähler antwortet mit falscher Adresse (13), ID 12000012
//! - Adr. 14 und 15: zwei Zähler mit derselben ID 77777777, verschiedene Hersteller
//! - Adr. 16: Wackelkontakt, antwortet nur auf jedes zweite Telegramm, ID 16000016
//! - Adr. 17 und 18: zwei Zähler mit derselben ID 88888888 und demselben Hersteller (echter Fehler)
//! - Adr. 19: Batteriezähler mit Kontingent, ID 19000019: liefert 2 Auslesungen, danach
//!   „zu viele Auslesungen“ (Fehlertelegramm CI 70, Code 09)
//!
//! Zähler hören nur Telegramme mit ihrer eigenen Baudrate. Baudratenwechsel (CI B8..BF)
//! und Application Reset (CI 50) werden quittiert.
//!
//! Das ist die Simulation „Praxis mit Fehlern“ (Anschluss `SIMULATOR`). Weitere Simulationen sind
//! Listen von [`SimMeter`]: eingebaute Vorlagen (z.B. „Alles in Ordnung – 10 Zähler“) und eigene,
//! in der Oberfläche angelegte (Anschluss `SIMULATOR:<name>`).

mod bus;
mod scenarios;
mod store;
#[cfg(test)]
mod tests;

pub use self::bus::*;
pub use self::scenarios::*;
pub use self::store::*;

use std::collections::VecDeque;
use std::io;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use chrono::{Datelike, Local, Timelike};

use crate::i18n::tr;
use crate::link::Link;
use crate::mbus::frame::{
    self, Frame, Parse, ACK, ADDR_BROADCAST, ADDR_BROADCAST_REPLY, ADDR_SELECTED, CI_APP_RESET,
    CI_DATA_SEND, CI_SELECT,
};
use crate::mbus::master::BAUD_RATES;
use crate::mbus::tables::manufacturer_code;
