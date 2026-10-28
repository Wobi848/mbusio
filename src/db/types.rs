//! Datentypen der Datenbank (Projekt, Gateway, Linie, Zähler, Knoten für den Baum, Pläne) und
//! wie sie aus einer Tabellenzeile entstehen.

use super::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub customer: String,
    #[serde(default)]
    pub site: String,
    #[serde(default)]
    pub technician: String,
    #[serde(default)]
    pub order_no: String,
    #[serde(default)]
    pub remarks: String,
    #[serde(default)]
    pub created: String,
    /// Ausleseplan als Vorgabe für alle Linien (siehe `plan::Plan`), Standard "off"
    #[serde(default = "default_plan")]
    pub plan: String,
    /// Verlauf ausdünnen: nach so vielen Tagen nur noch ein Wert pro Zähler und Tag; None = alles behalten
    #[serde(default)]
    pub thin_days: Option<i64>,
}

pub(super) fn default_plan() -> String {
    "off".into()
}

/// Gespeicherter Plan in der Normalform; Unbekanntes (z.B. aus einer Datei) fällt weg
pub(super) fn valid_plan(s: Option<&str>) -> Option<String> {
    Some(Plan::parse(s?).ok()?.key())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Gateway {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub project_id: i64,
    pub name: String,
    /// z.B. IP-Adresse; dient als Vorgabe für neue Linien
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Line {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub gateway_id: i64,
    pub name: String,
    /// wie beim Verbinden: COM3, /dev/ttyUSB0, tcp://192.168.1.50:10001 oder SIMULATOR; leer = noch nicht festgelegt
    #[serde(default)]
    pub connection: String,
    #[serde(default = "default_baud")]
    pub baud: u32,
    #[serde(default)]
    pub timeout_ms: u64,
    /// Wie Zähler dieser Linie gemerkt werden: "secondary" (SA) oder "primary" (PA)
    #[serde(default = "default_addressing")]
    pub addressing: String,
    /// Eigener Ausleseplan; None = wie Projekt
    #[serde(default)]
    pub plan: Option<String>,
    /// Dauerbetrieb eingeschaltet: nur dann liest der Plan und wird weitergegeben (§13).
    /// Wird nie importiert: eine übernommene Anlage liest erst, wenn man sie einschaltet.
    #[serde(default)]
    pub operation: bool,
}

/// Nur für Projekt-Exporte ohne das Feld: die stammen aus der Zeit, als über SA gemerkt wurde.
/// Neue Linien merken über PA (create_line).
fn default_addressing() -> String {
    "secondary".into()
}

fn default_baud() -> u32 {
    2400
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meter {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub line_id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub note: String,
    /// Wie beim Auslesen: Primäradresse, ID (8) oder Sekundäradresse (16)
    pub address: String,
    /// Eigene Baudrate, falls der Zähler nicht mit der Baudrate der Linie läuft
    #[serde(default)]
    pub baud: Option<u32>,
    #[serde(default)]
    pub meter_id: Option<String>,
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default)]
    pub medium: Option<String>,
    /// Primäradresse bei der letzten Auslesung
    #[serde(default)]
    pub primary_address: Option<u8>,
    /// Volle Sekundäradresse (16 Hex), sobald bekannt; erlaubt das Umstellen PA ↔ SA
    #[serde(default)]
    pub secondary_address: Option<String>,
    #[serde(default)]
    pub last_read: Option<String>,
    /// Hauptwert der letzten Auslesung, z.B. "123.456 m³"
    #[serde(default)]
    pub last_value: Option<String>,
    /// Schlüssel der Messgrösse zu `last_value`
    #[serde(default)]
    pub last_quantity: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    /// Eigener Ausleseplan; None = wie Linie
    #[serde(default)]
    pub plan: Option<String>,
    /// Zustand im Plan (nur lesen): Plan und zuletzt erledigter Termin dazu
    #[serde(default, skip_deserializing)]
    pub plan_key: Option<String>,
    #[serde(default, skip_deserializing)]
    pub plan_slot: Option<String>,
    /// ausgesetzt bis, weil der Zähler abgelehnt hat (beschäftigt, zu viele Auslesungen)
    #[serde(default, skip_deserializing)]
    pub plan_pause: Option<String>,
    /// Termine nacheinander nicht gelesen; ab 3 gilt der Zähler als gestört
    #[serde(default, skip_deserializing)]
    pub plan_fails: i64,
    /// gemessene Dauer einer Auslesung (gleitender Mittelwert), für die Buslast
    #[serde(default, skip_deserializing)]
    pub plan_ms: Option<f64>,
    /// Uhr im Zähler weicht so viele Minuten ab (ab 5 min, bei der letzten Auslesung)
    #[serde(default, skip_deserializing)]
    pub clock_off: Option<i64>,
    /// Fehler, den der Zähler bei der letzten Auslesung selbst meldet (`fault_of`)
    #[serde(default, skip_deserializing)]
    pub fault: Option<String>,
}

/// Abweichung der Uhr im Zähler von der eigenen Uhr in Minuten, ab `CLOCK_OFF_MIN`. Nur wenn die
/// eigene Uhr bestätigt ist; Stichtagswerte und Monatswerte hängen an der Zähleruhr.
pub(crate) fn clock_offset(r: &MeterReading) -> Option<i64> {
    if r.time_unsure {
        return None;
    }
    let ours = chrono::NaiveDateTime::parse_from_str(&r.timestamp, "%Y-%m-%d %H:%M:%S").ok()?;
    let rec = r
        .records
        .iter()
        .find(|x| x.quantity == "date_time" && x.storage == 0 && x.function == "instantaneous")?;
    let theirs = chrono::NaiveDateTime::parse_from_str(&rec.value_text, "%Y-%m-%d %H:%M").ok()?;
    let off = (theirs - ours).num_seconds() as f64 / 60.0;
    (off.abs() >= CLOCK_OFF_MIN as f64).then_some(off.round() as i64)
}

const CLOCK_OFF_MIN: i64 = 5;

/// Fehler, den der Zähler selbst meldet (Konzept Export 3.6): `status_error` (Statusbyte 0x08
/// dauerhaft, 0x10 vorübergehend), `error_flags` (Fehlerflags ungleich 0), `power_low` (0x04)
pub(crate) fn fault_of(r: &MeterReading) -> Option<&'static str> {
    let status = r.header.as_ref().map_or(0, |h| h.status);
    let flags = r.records.iter().any(|x| {
        x.quantity == "error_flags" && x.storage == 0 && x.value.as_f64().is_some_and(|v| v != 0.0)
    });
    if status & 0x18 != 0 {
        Some("status_error")
    } else if flags {
        Some("error_flags")
    } else if status & 0x04 != 0 {
        Some("power_low")
    } else {
        None
    }
}

/// Ab so vielen verpassten Terminen nacheinander gilt ein Zähler als gestört
pub const DISTURBED_AFTER: i64 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub time: String,
    #[serde(default)]
    pub line: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct LineInfo {
    #[serde(flatten)]
    pub line: Line,
    pub meters: i64,
    /// Zähler, die noch nie ausgelesen wurden (weder mit Wert noch mit Fehler)
    pub unread: i64,
    pub last_read: Option<String>,
    pub scanned: Option<String>,
    /// Auffälligkeiten des letzten Scans: {"count": n, "items": [...]}
    pub issues: Option<Value>,
    /// Ausleseplan der Linie mit nächstem Termin und letztem Ergebnis
    pub plan_info: Value,
    /// Datenpunkte der Linie: {"count": n, "invalid": n}
    pub datapoints: Value,
}

#[derive(Debug, Serialize)]
pub struct GatewayNode {
    #[serde(flatten)]
    pub gateway: Gateway,
    pub lines: Vec<LineInfo>,
}

#[derive(Debug, Serialize)]
pub struct ProjectNode {
    #[serde(flatten)]
    pub project: Project,
    pub gateways: Vec<GatewayNode>,
}

/// Eine Linie mit dem gültigen Plan je Zähler
pub struct PlannedLine {
    pub line: Line,
    /// "Gateway › Linie" fürs Protokoll
    pub label: String,
    pub meters: Vec<(Meter, Plan)>,
}

/// Ausgang eines Termins für einen Zähler
pub enum PlanOutcome {
    /// gelesen, so lange hat es gedauert
    Read { ms: f64 },
    /// nicht gelesen (auch nach Wiederholung)
    Failed,
    /// Zähler hat abgelehnt (beschäftigt, zu viele Auslesungen): ausgesetzt bis
    Refused { until: String },
}

/// Wo die aktive Linie hängt, für Kopfzeile und Bericht
#[derive(Debug, Clone, Serialize)]
pub struct Active {
    pub project: Project,
    pub gateway: Gateway,
    pub line: Line,
}

// ---------------------------------------------------------------- aus Tabellenzeilen

pub(super) fn project_of(r: &Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get("id")?,
        name: r.get("name")?,
        customer: r.get("customer")?,
        site: r.get("site")?,
        technician: r.get("technician")?,
        order_no: r.get("order_no")?,
        remarks: r.get("remarks")?,
        created: r.get("created")?,
        plan: r.get("plan")?,
        thin_days: r.get("thin_days")?,
    })
}

pub(super) fn gateway_of(r: &Row) -> rusqlite::Result<Gateway> {
    Ok(Gateway {
        id: r.get("id")?,
        project_id: r.get("project_id")?,
        name: r.get("name")?,
        address: r.get("address")?,
        note: r.get("note")?,
    })
}

pub(super) fn line_of(r: &Row) -> rusqlite::Result<Line> {
    Ok(Line {
        id: r.get("id")?,
        gateway_id: r.get("gateway_id")?,
        name: r.get("name")?,
        connection: r.get("connection")?,
        baud: r.get("baud")?,
        timeout_ms: r.get::<_, i64>("timeout_ms")? as u64,
        addressing: r.get("addressing")?,
        plan: r.get("plan")?,
        operation: r.get("operation")?,
    })
}

pub(super) fn meter_of(r: &Row) -> rusqlite::Result<Meter> {
    Ok(Meter {
        id: r.get("id")?,
        line_id: r.get("line_id")?,
        name: r.get("name")?,
        location: r.get("location")?,
        note: r.get("note")?,
        address: r.get("address")?,
        baud: r.get("baud")?,
        meter_id: r.get("meter_id")?,
        manufacturer: r.get("manufacturer")?,
        medium: r.get("medium")?,
        primary_address: r.get("primary_address")?,
        secondary_address: r.get("secondary_address")?,
        last_read: r.get("last_read")?,
        last_value: r.get("last_value")?,
        last_quantity: r.get("last_quantity")?,
        last_error: r.get("last_error")?,
        plan: r.get("plan")?,
        plan_key: r.get("plan_key")?,
        plan_slot: r.get("plan_slot")?,
        plan_pause: r.get("plan_pause")?,
        plan_fails: r.get("plan_fails")?,
        plan_ms: r.get("plan_ms")?,
        clock_off: r.get("clock_off")?,
        fault: r.get("fault")?,
    })
}
