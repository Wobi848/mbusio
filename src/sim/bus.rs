//! Der simulierte Bus: welche Zähler eine Anfrage hören, Antworten und Kollisionen, Baudraten,
//! Zeit für die Zählerstände. Die Zähler selbst stehen in `meter`.

use super::*;

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
