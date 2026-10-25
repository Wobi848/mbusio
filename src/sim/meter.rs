//! Ein simulierter Zähler: Fehlerarten, Modelle, Auswahl über die Sekundäradresse und wie er
//! seine Antworttelegramme baut.

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

pub(super) struct Slave {
    pub(super) primary: u8,
    pub(super) id: u32,
    pub(super) manufacturer: u16,
    pub(super) version: u8,
    pub(super) medium: u8,
    pub(super) model: Model,
    pub(super) fault: Fault,
    pub(super) access: u8,
    pub(super) baud: u32,
    pub(super) selected: bool,
    /// für Fault::Flaky: hat das letzte Telegramm gehört
    pub(super) heard_last: bool,
    pub(super) part: usize,
    pub(super) last_fcb: Option<bool>,
    /// für Fault::TooMany: bisherige Auslesungen
    pub(super) readouts: u32,
    /// Versatz der Werte, damit nicht alle Zähler gleich viel anzeigen
    pub(super) seed: u32,
}

impl Slave {
    pub(super) fn new(
        primary: u8,
        id: u32,
        man: &str,
        version: u8,
        medium: u8,
        model: Model,
    ) -> Self {
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

    pub(super) fn with_baud(mut self, baud: u32) -> Self {
        self.baud = baud;
        self
    }

    pub(super) fn with_fault(mut self, fault: Fault) -> Self {
        self.fault = fault;
        self
    }

    /// Kommt das Telegramm an? Bei Wackelkontakt nur jedes zweite.
    pub(super) fn hears(&mut self) -> bool {
        if self.fault != Fault::Flaky {
            return true;
        }
        self.heard_last = !self.heard_last;
        self.heard_last
    }

    pub(super) fn reset(&mut self) {
        self.part = 0;
        self.last_fcb = None;
    }

    pub(super) fn matches(&self, sel: &[u8]) -> bool {
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

    pub(super) fn rsp_ud(&mut self, fcb: bool, elapsed: f64) -> Frame {
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
