//! Verhalten der simulierten Zähler am Bus: Telegramme, Antworten, Fehler, Baudraten

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Fault {
    None,
    Checksum,
    Truncated,
    /// antwortet mit Fehlertelegramm CI 70 "beschäftigt"
    Busy,
    /// trägt die falsche Primäradresse ins Antworttelegramm ein
    WrongAddress,
    /// Wackelkontakt: jedes zweite Telegramm kommt nicht an
    Flaky,
    /// Kontingent aufgebraucht: nach 2 Auslesungen Fehlertelegramm CI 70 "zu viele Auslesungen"
    TooMany,
}

#[derive(Clone, Copy)]
pub(super) enum Model {
    Heat,
    Electricity,
    Water,
    /// Feste Datenstruktur (CI 73), wie sehr alte Zähler
    Fixed,
}

struct Slave {
    primary: u8,
    id: u32,
    manufacturer: u16,
    version: u8,
    medium: u8,
    model: Model,
    fault: Fault,
    access: u8,
    baud: u32,
    selected: bool,
    /// für Fault::Flaky: hat das letzte Telegramm gehört
    heard_last: bool,
    part: usize,
    last_fcb: Option<bool>,
    /// für Fault::TooMany: bisherige Auslesungen
    readouts: u32,
    /// Versatz der Werte, damit nicht alle Zähler gleich viel anzeigen
    seed: u32,
}

impl Slave {
    fn new(primary: u8, id: u32, man: &str, version: u8, medium: u8, model: Model) -> Self {
        Slave {
            primary,
            id,
            manufacturer: manufacturer_code(man),
            version,
            medium,
            model,
            fault: Fault::None,
            access: 0,
            baud: 0,
            selected: false,
            // so beginnt der Wackelkontakt mit einem verpassten Telegramm
            heard_last: true,
            part: 0,
            last_fcb: None,
            readouts: 0,
            seed: 0,
        }
    }

    fn with_baud(mut self, baud: u32) -> Self {
        self.baud = baud;
        self
    }

    fn with_fault(mut self, fault: Fault) -> Self {
        self.fault = fault;
        self
    }

    /// Kommt das Telegramm an? Bei Wackelkontakt nur jedes zweite.
    fn hears(&mut self) -> bool {
        if self.fault != Fault::Flaky {
            return true;
        }
        self.heard_last = !self.heard_last;
        self.heard_last
    }

    fn reset(&mut self) {
        self.part = 0;
        self.last_fcb = None;
    }

    fn matches(&self, sel: &[u8]) -> bool {
        if sel.len() < 8 {
            return false;
        }
        let id = self.id.to_le_bytes();
        let id_ok = (0..4).all(|k| {
            let (m, v) = (sel[k], id[k]);
            (m >> 4 == 0xF || m >> 4 == v >> 4) && (m & 0xF == 0xF || m & 0xF == v & 0xF)
        });
        let man = u16::from_le_bytes([sel[4], sel[5]]);
        id_ok
            && (man == 0xFFFF || man == self.manufacturer)
            && (sel[6] == 0xFF || sel[6] == self.version)
            && (sel[7] == 0xFF || sel[7] == self.medium)
    }

    fn rsp_ud(&mut self, fcb: bool, elapsed: f64) -> Frame {
        let parts = match self.model {
            Model::Heat => 2,
            _ => 1,
        };
        match self.last_fcb {
            None => self.part = 0,
            Some(last) if last != fcb => self.part = (self.part + 1) % parts,
            Some(_) => {} // gleicher FCB: Wiederholung
        }
        self.last_fcb = Some(fcb);
        self.access = self.access.wrapping_add(1);

        if self.fault == Fault::Busy {
            return Frame::Long {
                control: 0x08,
                address: self.primary,
                ci: 0x70,
                data: vec![0x08],
            };
        }
        if self.fault == Fault::TooMany {
            self.readouts += 1;
            if self.readouts > 2 {
                return Frame::Long {
                    control: 0x08,
                    address: self.primary,
                    ci: 0x70,
                    data: vec![0x09],
                };
            }
        }

        if let Model::Fixed = self.model {
            // ID, Zugriffsnummer, Status (BCD, aktuelle Werte), Medium/Einheiten, 2 Zähler
            let mut d = self.id.to_le_bytes().to_vec();
            // Medium Wärme (4): Bits 7..6 beider Einheitenbytes; Einheiten kWh (05) und m³ (2C)
            d.extend_from_slice(&[self.access, 0x00, 0x05, 0x40 | 0x2C]);
            d.extend_from_slice(&bcd_bytes(
                12_345 + self.seed as u64 + (elapsed as u64) / 60,
                4,
            ));
            d.extend_from_slice(&bcd_bytes(678, 4));
            return Frame::Long {
                control: 0x08,
                address: self.primary,
                ci: 0x73,
                data: d,
            };
        }

        let mut d = Vec::new();
        d.extend_from_slice(&self.id.to_le_bytes());
        d.extend_from_slice(&self.manufacturer.to_le_bytes());
        d.extend_from_slice(&[self.version, self.medium, self.access, 0x00, 0x00, 0x00]);
        let tick = elapsed as u32;
        let seed = self.seed;
        match (self.model, self.part) {
            (Model::Heat, 0) => {
                rec(
                    &mut d,
                    &[0x04, 0x06],
                    &(123_456 + seed + tick / 10).to_le_bytes(),
                ); // kWh
                rec(
                    &mut d,
                    &[0x04, 0x14],
                    &(456_789 + seed * 3 + tick / 4).to_le_bytes(),
                ); // 0.01 m³
                rec(&mut d, &[0x04, 0x2B], &(12_345 + tick % 50).to_le_bytes()); // W
                rec(&mut d, &[0x02, 0x59], &6523i16.to_le_bytes()); // 0.01 °C
                rec(&mut d, &[0x02, 0x5D], &4210i16.to_le_bytes());
                rec(&mut d, &[0x02, 0x61], &2313i16.to_le_bytes()); // 0.01 K
                rec(&mut d, &[0x04, 0x3B], &1234u32.to_le_bytes()); // l/h
                rec(&mut d, &[0x04, 0x6D], &now_type_f());
                rec(&mut d, &[0x44, 0x06], &120_000u32.to_le_bytes()); // Stichtag
                rec(&mut d, &[0x42, 0x6C], &type_g(2026, 9, 30));
                rec(&mut d, &[0x01, 0xFD, 0x17], &[0x00]);
                rec(&mut d, &[0x0C, 0x78], &self.id.to_le_bytes());
                d.push(0x1F); // weitere Daten folgen
            }
            (Model::Heat, _) => {
                rec(&mut d, &[0x84, 0x01, 0x06], &110_000u32.to_le_bytes());
                rec(&mut d, &[0x82, 0x01, 0x6C], &type_g(2025, 9, 30));
                d.extend_from_slice(&[0x0F, 0x01, 0x02, 0x03]);
            }
            (Model::Electricity, _) => {
                rec(
                    &mut d,
                    &[0x0E, 0x03],
                    &bcd_bytes(12_345_678 + seed as u64 * 1000 + tick as u64, 6),
                ); // Wh
                rec(&mut d, &[0x8E, 0x10, 0x03], &bcd_bytes(8_000_000, 6));
                rec(&mut d, &[0x8E, 0x20, 0x03], &bcd_bytes(4_345_678, 6));
                rec(&mut d, &[0x04, 0x2B], &(2_300 + tick % 100).to_le_bytes());
                rec(&mut d, &[0x05, 0x2E], &3.5f32.to_le_bytes()); // kW, real
                rec(&mut d, &[0x04, 0xFD, 0x48], &2301u32.to_le_bytes()); // 0.1 V
                rec(&mut d, &[0x04, 0xFD, 0x59], &5123u32.to_le_bytes()); // mA
                let fw: Vec<u8> = b"1.02".iter().rev().copied().collect();
                rec(&mut d, &[0x0D, 0xFD, 0x0E, fw.len() as u8], &fw);
            }
            (Model::Water | Model::Fixed, _) => {
                rec(
                    &mut d,
                    &[0x0C, 0x13],
                    &bcd_bytes(123_456 + seed as u64 * 7 + tick as u64, 4),
                ); // l
                rec(&mut d, &[0x4C, 0x13], &bcd_bytes(100_000, 4));
                rec(&mut d, &[0x42, 0x6C], &type_g(2025, 12, 31));
                rec(&mut d, &[0x04, 0x6D], &now_type_f());
            }
        }
        Frame::Long {
            control: 0x08,
            address: if self.fault == Fault::WrongAddress {
                self.primary.wrapping_add(1)
            } else {
                self.primary
            },
            ci: 0x72,
            data: d,
        }
    }
}

fn rec(d: &mut Vec<u8>, head: &[u8], value: &[u8]) {
    d.extend_from_slice(head);
    d.extend_from_slice(value);
}

fn bcd_bytes(mut v: u64, n: usize) -> Vec<u8> {
    (0..n)
        .map(|_| {
            let lo = (v % 10) as u8;
            v /= 10;
            let hi = (v % 10) as u8;
            v /= 10;
            (hi << 4) | lo
        })
        .collect()
}

fn type_g(year: u32, month: u8, day: u8) -> [u8; 2] {
    let y = (year % 100) as u8;
    [day | ((y & 0x07) << 5), month | ((y >> 3) << 4)]
}

fn now_type_f() -> [u8; 4] {
    let t = Local::now();
    let g = type_g(t.year() as u32, t.month() as u8, t.day() as u8);
    [t.minute() as u8, t.hour() as u8, g[0], g[1]]
}

/// Ab hier laufen die Zählerstände (1.10.2026, Sekunden seit 1970)
const COUNTER_ORIGIN: f64 = 1_790_812_800.0;

/// Sekunden seit 1970 nach der Systemuhr (nicht der Uhr von mbusio: die kann ein Leitsystem
/// verstellen, und rückwärts würde ein Zählerstand sinken)
fn system_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

pub struct Simulator {
    slaves: Vec<Slave>,
    baud: u32,
    inbuf: Vec<u8>,
    rx: VecDeque<u8>,
    byte_time: Duration,
    /// Zählerstände = Anfangswert + Zeit seit `origin`. Fest, damit sie auch über einen Neustart
    /// von mbusio weiter steigen wie bei echten Zählern (sonst „Zählerstand gesunken“); in Tests
    /// ab dem Start, damit die Anfangswerte fest sind.
    origin: f64,
}

impl Simulator {
    /// Simulation „Praxis mit Fehlern“ (Anschluss `SIMULATOR`)
    pub fn new(baud: u32) -> Self {
        Self::from_meters(baud, &praxis(), true)
    }

    /// Simulator aus einer Liste von Zählern. `praxis`: Zähler mit eigener Baudrate laufen auf
    /// 2400 Bd, falls der Bus selbst mit ihrer Baudrate läuft (so bleibt immer einer „anders“),
    /// und alle Werte beginnen gleich (für die Tests).
    pub fn from_meters(baud: u32, meters: &[SimMeter], praxis: bool) -> Self {
        let slaves = meters
            .iter()
            .map(|m| {
                let id = u32::from_str_radix(&m.id, 16).unwrap_or(0);
                let mut s = Slave::new(
                    m.primary,
                    id,
                    &m.manufacturer,
                    m.version,
                    medium_code(&m.medium),
                    model_of(&m.medium),
                )
                .with_fault(fault_of(&m.fault))
                .with_baud(match m.baud {
                    Some(b) if praxis && b == baud => 2400,
                    Some(b) => b,
                    None => baud,
                });
                if !praxis {
                    // Zählerstände je Zähler verschieden, aber immer gleich für dieselbe ID
                    s.seed = (m.id.parse::<u64>().unwrap_or(0) % 997 * 53) as u32;
                }
                s
            })
            .collect();
        Simulator {
            slaves,
            baud,
            inbuf: Vec::new(),
            rx: VecDeque::new(),
            byte_time: Duration::from_secs_f64(11.0 / baud.max(300) as f64),
            origin: if cfg!(test) {
                system_secs()
            } else {
                COUNTER_ORIGIN
            },
        }
    }

    /// Tests: Zählerstände ab `origin` laufen lassen wie im Programm (statt ab dem Start)
    #[cfg(test)]
    pub fn with_counter_origin(mut self) -> Self {
        self.origin = COUNTER_ORIGIN;
        self
    }

    fn targets(&self, address: u8) -> Vec<usize> {
        (0..self.slaves.len())
            .filter(|&k| self.slaves[k].baud == self.baud)
            .filter(|&k| match address {
                ADDR_BROADCAST | ADDR_BROADCAST_REPLY => true,
                ADDR_SELECTED => self.slaves[k].selected,
                a => self.slaves[k].primary == a,
            })
            .collect()
    }

    fn handle(&mut self, f: Frame) -> Vec<u8> {
        // vor dem Ursprung (Uhr noch nicht gestellt, z.B. Pi ohne Netz): Anfangswerte
        let elapsed = (system_secs() - self.origin).max(0.0);
        let mut replies: Vec<Vec<u8>> = Vec::new();
        match f {
            Frame::Short { control, address } if control == frame::C_SND_NKE => {
                for k in self.targets(address) {
                    let s = &mut self.slaves[k];
                    if !s.hears() {
                        continue;
                    }
                    s.reset();
                    if address == ADDR_SELECTED || address == ADDR_BROADCAST {
                        s.selected = false;
                    }
                    replies.push(vec![ACK]);
                }
            }
            Frame::Short { control, address } if control & 0xCF == 0x4B => {
                let fcb = control & frame::FCB != 0;
                for k in self.targets(address) {
                    let slave = &mut self.slaves[k];
                    if !slave.hears() {
                        continue;
                    }
                    let mut bytes = slave.rsp_ud(fcb, elapsed).encode();
                    match slave.fault {
                        Fault::None
                        | Fault::Busy
                        | Fault::WrongAddress
                        | Fault::Flaky
                        | Fault::TooMany => {}
                        Fault::Checksum => {
                            let n = bytes.len();
                            bytes[n - 2] ^= 0x5A;
                        }
                        Fault::Truncated => bytes.truncate(bytes.len() / 2),
                    }
                    replies.push(bytes);
                }
            }
            Frame::Long {
                control,
                address,
                ci,
                data,
            } if control & 0xCF == 0x43 => {
                if ci == CI_SELECT && address == ADDR_SELECTED {
                    let baud = self.baud;
                    for s in self.slaves.iter_mut().filter(|s| s.baud == baud) {
                        s.selected = s.matches(&data) && s.hears();
                        if s.selected {
                            s.reset();
                            replies.push(vec![ACK]);
                        }
                    }
                } else if ci == CI_DATA_SEND
                    && data.len() == 3
                    && data[0] == 0x01
                    && data[1] == 0x7A
                {
                    for k in self.targets(address) {
                        self.slaves[k].primary = data[2];
                        replies.push(vec![ACK]);
                    }
                } else if (0xB8..=0xBF).contains(&ci) && data.is_empty() {
                    let new = BAUD_RATES[(ci - 0xB8) as usize];
                    for k in self.targets(address) {
                        // ACK geht noch mit der alten Baudrate raus
                        self.slaves[k].baud = new;
                        replies.push(vec![ACK]);
                    }
                } else if ci == CI_APP_RESET {
                    for k in self.targets(address) {
                        self.slaves[k].reset();
                        replies.push(vec![ACK]);
                    }
                }
            }
            _ => {}
        }
        match replies.len() {
            0 => Vec::new(),
            1 => replies.pop().unwrap(),
            // Mehrere Sender: ACKs folgen aufeinander, Datentelegramme überlagern sich
            _ if replies.iter().all(|r| r == &[ACK]) => replies.concat(),
            _ => {
                let len = replies.iter().map(Vec::len).max().unwrap();
                (0..len)
                    .map(|k| {
                        replies
                            .iter()
                            .fold(0xFF, |acc, r| acc & r.get(k).copied().unwrap_or(0xFF))
                    })
                    .collect()
            }
        }
    }
}

impl Link for Simulator {
    fn send(&mut self, data: &[u8]) -> io::Result<()> {
        self.inbuf.extend_from_slice(data);
        loop {
            match frame::parse(&self.inbuf) {
                Parse::Complete(f, used) => {
                    self.inbuf.drain(..used);
                    let broadcast = matches!(
                        f,
                        Frame::Short {
                            address: ADDR_BROADCAST,
                            ..
                        }
                    ) || matches!(
                        f,
                        Frame::Long {
                            address: ADDR_BROADCAST,
                            ..
                        }
                    );
                    let reply = self.handle(f);
                    if !broadcast {
                        self.rx.extend(reply);
                    }
                }
                Parse::Incomplete => break,
                Parse::Invalid(_) => {
                    self.inbuf.clear();
                    break;
                }
            }
        }
        Ok(())
    }

    fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        if self.rx.is_empty() {
            std::thread::sleep(timeout.min(Duration::from_millis(15)));
            return Ok(0);
        }
        let n = buf.len().min(self.rx.len()).min(32);
        for b in buf.iter_mut().take(n) {
            *b = self.rx.pop_front().unwrap();
        }
        // Übertragungszeit grob nachbilden, damit sich der Simulator wie ein Bus anfühlt
        std::thread::sleep(self.byte_time * n as u32 / 4);
        Ok(n)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.rx.clear();
        Ok(())
    }

    fn set_baud(&mut self, baud: u32) -> io::Result<()> {
        self.baud = baud;
        self.byte_time = Duration::from_secs_f64(11.0 / baud.max(300) as f64);
        self.inbuf.clear();
        Ok(())
    }
}
