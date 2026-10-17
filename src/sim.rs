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
//! Zähler hören nur Telegramme mit ihrer eigenen Baudrate. Baudratenwechsel (CI B8..BF)
//! und Application Reset (CI 50) werden quittiert.

use std::collections::VecDeque;
use std::io;
use std::time::{Duration, Instant};

use chrono::{Datelike, Local, Timelike};

use crate::link::Link;
use crate::mbus::frame::{
    self, Frame, Parse, ACK, ADDR_BROADCAST, ADDR_BROADCAST_REPLY, ADDR_SELECTED, CI_APP_RESET,
    CI_DATA_SEND, CI_SELECT,
};
use crate::mbus::master::BAUD_RATES;
use crate::mbus::tables::manufacturer_code;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fault {
    None,
    Checksum,
    Truncated,
}

#[derive(Clone, Copy)]
enum Model {
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
    part: usize,
    last_fcb: Option<bool>,
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
            part: 0,
            last_fcb: None,
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

        if let Model::Fixed = self.model {
            // ID, Zugriffsnummer, Status (BCD, aktuelle Werte), Medium/Einheiten, 2 Zähler
            let mut d = self.id.to_le_bytes().to_vec();
            // Medium Wärme (4): Bits 7..6 beider Einheitenbytes; Einheiten kWh (05) und m³ (2C)
            d.extend_from_slice(&[self.access, 0x00, 0x05, 0x40 | 0x2C]);
            d.extend_from_slice(&bcd_bytes(12_345 + (elapsed as u64) / 60, 4));
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
        match (self.model, self.part) {
            (Model::Heat, 0) => {
                rec(&mut d, &[0x04, 0x06], &(123_456 + tick / 10).to_le_bytes()); // kWh
                rec(&mut d, &[0x04, 0x14], &(456_789 + tick / 4).to_le_bytes()); // 0.01 m³
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
                    &bcd_bytes(12_345_678 + tick as u64, 6),
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
                rec(&mut d, &[0x0C, 0x13], &bcd_bytes(123_456 + tick as u64, 4)); // l
                rec(&mut d, &[0x4C, 0x13], &bcd_bytes(100_000, 4));
                rec(&mut d, &[0x42, 0x6C], &type_g(2025, 12, 31));
                rec(&mut d, &[0x04, 0x6D], &now_type_f());
            }
        }
        Frame::Long {
            control: 0x08,
            address: self.primary,
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

pub struct Simulator {
    slaves: Vec<Slave>,
    baud: u32,
    inbuf: Vec<u8>,
    rx: VecDeque<u8>,
    byte_time: Duration,
    start: Instant,
}

impl Simulator {
    pub fn new(baud: u32) -> Self {
        let mut slaves = vec![
            Slave::new(1, 0x1234_5678, "KAM", 0x1B, 0x04, Model::Heat),
            Slave::new(2, 0x8765_4321, "ABB", 0x02, 0x02, Model::Electricity),
            Slave::new(3, 0x3000_0001, "SEN", 0x10, 0x07, Model::Water),
            Slave::new(0, 0x1234_5699, "SEN", 0x10, 0x16, Model::Water),
            Slave::new(0, 0x5550_0011, "ZRI", 0x01, 0x06, Model::Water),
            Slave::new(4, 0x4000_0004, "SEN", 0x10, 0x07, Model::Water).with_fault(Fault::Checksum),
            Slave::new(5, 0x5000_0005, "SEN", 0x10, 0x07, Model::Water)
                .with_fault(Fault::Truncated),
            Slave::new(7, 0x7000_0007, "", 0, 0x04, Model::Fixed),
        ];
        for s in &mut slaves {
            s.baud = baud;
        }
        let other = if baud == 9600 { 2400 } else { 9600 };
        slaves.push(Slave::new(6, 0x6000_0006, "SEN", 0x10, 0x07, Model::Water).with_baud(other));
        Simulator {
            slaves,
            baud,
            inbuf: Vec::new(),
            rx: VecDeque::new(),
            byte_time: Duration::from_secs_f64(11.0 / baud.max(300) as f64),
            start: Instant::now(),
        }
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
        let elapsed = self.start.elapsed().as_secs_f64();
        let mut replies: Vec<Vec<u8>> = Vec::new();
        match f {
            Frame::Short { control, address } if control == frame::C_SND_NKE => {
                for k in self.targets(address) {
                    let s = &mut self.slaves[k];
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
                    let mut bytes = slave.rsp_ud(fcb, elapsed).encode();
                    match slave.fault {
                        Fault::None => {}
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
                        s.selected = s.matches(&data);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mbus::master::{FrameLog, Master, Progress, Target};
    use std::sync::Arc;

    fn master() -> Master {
        Master::new(
            Box::new(Simulator::new(9600)),
            9600,
            50,
            Arc::new(FrameLog::default()),
        )
    }

    #[test]
    fn primary_scan_finds_meters_and_collision() {
        let mut m = master();
        let mut hits = Vec::new();
        m.scan_primary(0, 5, 0, &mut |p| {
            if let Progress::Hit(h) = p {
                hits.push(h);
            }
            true
        })
        .unwrap();
        let addrs: Vec<_> = hits
            .iter()
            .map(|h| (h.primary_address.unwrap(), h.status))
            .collect();
        assert_eq!(
            addrs,
            vec![
                (0, "collision"),
                (1, "ok"),
                (2, "ok"),
                (3, "ok"),
                (4, "no_data"),
                (5, "no_data")
            ]
        );
        assert_eq!(hits[1].id.as_deref(), Some("12345678"));
        assert_eq!(hits[1].manufacturer.as_deref(), Some("KAM"));
    }

    #[test]
    fn secondary_scan_resolves_all() {
        let mut m = master();
        let mut hits = Vec::new();
        m.scan_secondary(&mut |p| {
            if let Progress::Hit(h) = p {
                hits.push((h.id.clone().unwrap_or_default(), h.status));
            }
            true
        })
        .unwrap();
        hits.sort();
        assert_eq!(
            hits,
            vec![
                ("12345678".into(), "ok"),
                ("12345699".into(), "ok"),
                ("30000001".into(), "ok"),
                // fehlerhafte Telegramme: gefunden, aber ohne Daten
                ("40000004".into(), "no_data"),
                ("50000005".into(), "no_data"),
                ("55500011".into(), "ok"),
                ("70000007".into(), "ok"),
                // 60000006 läuft mit anderer Baudrate und fehlt deshalb
                ("87654321".into(), "ok"),
            ]
        );
    }

    #[test]
    fn read_multi_telegram() {
        let mut m = master();
        let r = m.read_meter(&Target::Primary(1)).unwrap();
        assert_eq!(r.raw_frames.len(), 2);
        assert!(r.records.iter().any(|x| x.storage == 2 && x.unit == "kWh"));
        assert!(r
            .records
            .iter()
            .any(|x| x.quantity == "flow_temperature" && x.value_text == "65.23"));
    }

    #[test]
    fn read_electricity_values() {
        let mut m = master();
        let r = m.read_meter(&Target::Primary(2)).unwrap();
        let find = |d: &str| {
            r.records
                .iter()
                .find(|x| x.quantity == d)
                .unwrap()
                .value_text
                .clone()
        };
        assert_eq!(find("voltage"), "230.1");
        assert_eq!(find("current"), "5.123");
        assert_eq!(find("firmware_version"), "1.02");
        assert!(r.records.iter().any(|x| x.tariff == 2));
    }

    #[test]
    fn corrupt_telegrams_are_rejected() {
        let mut m = master();
        let e = m.read_meter(&Target::Primary(4)).unwrap_err().to_string();
        assert!(e.contains("Kollision"), "{e}");
        assert!(m.read_meter(&Target::Primary(5)).is_err());
    }

    #[test]
    fn set_address_via_secondary() {
        let mut m = master();
        m.set_primary(&Target::parse("12345699").unwrap(), 10)
            .unwrap();
        let r = m.read_meter(&Target::Primary(10)).unwrap();
        assert_eq!(r.header.unwrap().id, "12345699");
        // Adresse 0 ist jetzt eindeutig
        let r = m.read_meter(&Target::Primary(0)).unwrap();
        assert_eq!(r.header.unwrap().id, "55500011");
    }

    #[test]
    fn set_address_refuses_collision() {
        let mut m = master();
        let e = m.set_primary(&Target::Primary(0), 9).unwrap_err().to_string();
        assert!(e.contains("Mehrere Zähler"), "{e}");
        // keiner der beiden wurde umgestellt
        assert!(m.read_meter(&Target::Primary(9)).is_err());
        let r = m.read_meter(&Target::parse("55500011").unwrap()).unwrap();
        assert_eq!(r.primary_address, 0);
    }

    #[test]
    fn set_address_refuses_occupied() {
        let mut m = master();
        let e = m.set_primary(&Target::Primary(1), 2).unwrap_err().to_string();
        assert!(e.contains("schon belegt"), "{e}");
        let r = m.read_meter(&Target::Primary(1)).unwrap();
        assert_eq!(r.header.unwrap().id, "12345678");
        let r = m.read_meter(&Target::Primary(2)).unwrap();
        assert_eq!(r.header.unwrap().id, "87654321");
    }

    #[test]
    fn set_address_to_own_address_is_ok() {
        let mut m = master();
        m.set_primary(&Target::Primary(1), 1).unwrap();
        m.set_primary(&Target::parse("87654321").unwrap(), 2).unwrap();
    }

    #[test]
    fn detect_and_switch_baud() {
        let mut m = master();
        // Adresse 6 läuft mit 2400 Bd, der Master mit 9600
        assert!(m.read_meter(&Target::Primary(6)).is_err());
        assert_eq!(m.detect_baud(&Target::Primary(6)).unwrap(), vec![2400]);
        assert_eq!(m.baud(), 9600);
        m.set_baud(2400).unwrap();
        m.switch_baud(&Target::parse("60000006").unwrap(), 9600)
            .unwrap();
        m.set_baud(9600).unwrap();
        let r = m.read_meter(&Target::Primary(6)).unwrap();
        assert_eq!(r.header.unwrap().id, "60000006");
        // Zähler 1 läuft weiter mit 9600
        assert_eq!(m.detect_baud(&Target::Primary(1)).unwrap(), vec![9600]);
    }

    #[test]
    fn switch_baud_refuses_collision() {
        let mut m = master();
        assert!(m.switch_baud(&Target::Primary(0), 2400).is_err());
        assert_eq!(m.detect_baud(&Target::Primary(0)).unwrap(), vec![9600]);
    }

    #[test]
    fn application_reset_is_acknowledged() {
        let mut m = master();
        m.application_reset(&Target::Primary(1), None).unwrap();
        m.application_reset(&Target::parse("87654321").unwrap(), Some(0x10))
            .unwrap();
        assert!(m.application_reset(&Target::Primary(99), None).is_err());
    }

    #[test]
    fn read_fixed_structure() {
        let mut m = master();
        let r = m.read_meter(&Target::Primary(7)).unwrap();
        let h = r.header.unwrap();
        assert_eq!((h.id.as_str(), h.medium.as_str()), ("70000007", "medium_heat_outlet"));
        assert_eq!(r.records.len(), 2);
        assert_eq!(
            (r.records[0].quantity.as_str(), r.records[0].unit.as_str()),
            ("energy", "kWh")
        );
        assert!(r.records[0].value_text.starts_with("123"));
        assert_eq!(
            (r.records[1].unit.as_str(), r.records[1].value_text.as_str()),
            ("m³", "678")
        );
    }

    #[test]
    fn raw_send_returns_reply() {
        let mut m = master();
        let ack = m.raw_send(&[0x10, 0x40, 0x01, 0x41, 0x16]).unwrap();
        assert!(matches!(ack, crate::mbus::master::Reply::Frame(crate::mbus::frame::Frame::Ack)));
        let none = m.raw_send(&[0x10, 0x40, 0x63, 0xA3, 0x16]).unwrap();
        assert!(matches!(none, crate::mbus::master::Reply::None));
    }
}
