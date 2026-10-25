//! Ausgang Modbus TCP: Startregister je Datenpunkt und Liste für den Integrator. Der Server selbst
//! (`crate::modbus`) liest den aktuellen Stand; zu senden gibt es nichts.

use super::*;
use crate::modbus::{block_start, BLOCK, STATUS_START};

pub struct Modbus;

impl Output for Modbus {
    fn key(&self) -> &'static str {
        "modbus"
    }

    fn label(&self) -> String {
        "Modbus".into()
    }

    fn address_label(&self) -> String {
        tr("Register", "Register")
    }

    /// Startregister: Vielfaches von 10 unter 60000 (so überlappen Blöcke nie)
    fn check_address(&self, address: &str) -> Result<()> {
        let ok = address
            .parse::<u16>()
            .is_ok_and(|a| a % BLOCK == 0 && a <= STATUS_START - BLOCK);
        if !ok {
            anyhow::bail!(
                "{}: „{address}“",
                tr(
                    "Register ungültig (Vielfaches von 10, höchstens 59990)",
                    "Invalid register (multiple of 10, at most 59990)"
                )
            );
        }
        Ok(())
    }

    /// Aus der Nummer, die im Projekt nie neu vergeben wird: Nummer 1 → 0, Nummer 7 → 60
    fn pattern_address(&self, _db: &Db, _place: &Place, d: &Datapoint) -> Result<String> {
        block_start(d.no).map(|a| a.to_string()).ok_or_else(|| {
            anyhow::anyhow!(tr("Keine freien Register mehr", "No free registers left"))
        })
    }

    fn is_on(&self, db: &Db) -> bool {
        db.modbus_config().enabled
    }

    fn deliver(&self, _db: &mut Db, _d: &Datapoint, _address: &str, _s: &Sample) -> Result<()> {
        Ok(())
    }

    /// Ein Server je Projekt: Adressen müssen nur im Projekt eindeutig sein
    fn global_addresses(&self) -> bool {
        false
    }

    fn list_info(&self, db: &Db) -> Option<Value> {
        let c = db.modbus_config();
        Some(json!({
            "port": c.port,
            "functions": tr("03 und 04 (dieselben Register), nur lesen", "03 and 04 (same registers), read only"),
            "unit_id": tr("jede", "any"),
            "word_order": if c.swap_words { "CDAB" } else { "ABCD" },
            "block": {
                "registers": BLOCK,
                "+0": "float64",
                "+4": "float32",
                "+6": tr("int32, Wert × 100", "int32, value × 100"),
                "+8": tr("Qualität: 0 gut, 1 veraltet, 2 gestört, 3 ungültig, 4 kein Wert",
                         "quality: 0 good, 1 stale, 2 disturbed, 3 invalid, 4 no value"),
                "+9": tr("Alter in Minuten", "age in minutes"),
            },
            "status": {
                "60000": tr("Lebenszeichen: Minuten seit dem Start", "heartbeat: minutes since start"),
                "60001": tr("Anzahl gestört oder ungültig", "number disturbed or invalid"),
                "60002": tr("Anzahl veraltet", "number stale"),
            },
        }))
    }
}
