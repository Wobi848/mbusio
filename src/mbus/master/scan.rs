//! Bus durchsuchen: Primär- und Sekundärscan, Baudraten der Geräte am Bus

use anyhow::Result;

use super::*;

impl Master {
    /// Auf welchen Baudraten antworten überhaupt Geräte? Je Baudrate ein Broadcast-Ping
    /// (alle Zähler quittieren) und eine Sekundär-Selektion mit lauter Platzhaltern.
    /// `bauds` leer = alle Standard-Baudraten. Die Oberfläche fragt einzeln ab, um den
    /// Fortschritt zu zeigen.
    pub fn bus_bauds(&mut self, bauds: &[u32]) -> Result<Vec<u32>> {
        let bauds = if bauds.is_empty() {
            &BAUD_RATES[..]
        } else {
            bauds
        };
        check_baud(bauds.iter().copied())?;
        let old = self.baud;
        self.link.set_baud(old)?;
        let result = self.probe_bus_bauds(bauds);
        self.set_baud(old)?;
        result
    }

    fn probe_bus_bauds(&mut self, bauds: &[u32]) -> Result<Vec<u32>> {
        let mut found = Vec::new();
        for &baud in bauds {
            self.set_baud(baud)?;
            let any = !matches!(self.ping(ADDR_BROADCAST_REPLY)?, Reply::None)
                || !matches!(self.select("FFFFFFFFFFFFFFFF")?, Reply::None);
            // Selektion wieder aufheben
            self.send(&Frame::snd_nke(ADDR_BROADCAST))?;
            std::thread::sleep(self.inter_byte);
            if any {
                found.push(baud);
            }
        }
        Ok(found)
    }

    /// Führt `scan` nacheinander auf mehreren Baudraten aus. Treffer tragen ihre Baudrate,
    /// der Fortschritt läuft über alle Baudraten; danach gilt wieder die alte Baudrate.
    pub fn scan_bauds(
        &mut self,
        bauds: &[u32],
        progress: ProgressFn,
        scan: &mut dyn FnMut(&mut Master, ProgressFn) -> Result<()>,
    ) -> Result<()> {
        let home = self.baud;
        let n = bauds.len() as u32;
        let mut result = Ok(());
        for (k, &baud) in bauds.iter().enumerate() {
            if let Err(e) = self.set_baud(baud) {
                result = Err(e);
                break;
            }
            let mut cancelled = false;
            let mut wrapped = |p: Progress| {
                let p = match p {
                    Progress::Step {
                        done,
                        total,
                        message,
                    } => Progress::Step {
                        done: k as u32 * total + done,
                        total: total * n,
                        message: if n > 1 {
                            format!("{baud} Bd · {message}")
                        } else {
                            message
                        },
                    },
                    Progress::Hit(mut h) => {
                        h.baud = Some(baud);
                        Progress::Hit(h)
                    }
                    Progress::Stopped { mask, .. } => Progress::Stopped {
                        mask,
                        baud: Some(baud),
                    },
                };
                let go = progress(p);
                cancelled |= !go;
                go
            };
            result = scan(self, &mut wrapped);
            if result.is_err() || cancelled {
                break;
            }
        }
        let restore = self.set_baud(home);
        result.and(restore)
    }

    /// Scan auf den gewählten Baudraten (für Busse mit gemischten Baudraten).
    pub fn scan_with(
        &mut self,
        bauds: Bauds,
        progress: ProgressFn,
        scan: &mut dyn FnMut(&mut Master, ProgressFn) -> Result<()>,
    ) -> Result<()> {
        let mut list = match bauds {
            Bauds::Current => vec![self.baud],
            Bauds::List(list) => {
                check_baud(list.iter().copied())?;
                list
            }
            Bauds::Probe => {
                if !progress(Progress::Step {
                    done: 0,
                    total: 1,
                    message: tr(
                        "Prüfe, auf welchen Baudraten Geräte antworten",
                        "Checking which baud rates have devices",
                    ),
                }) {
                    return Ok(());
                }
                self.bus_bauds(&[])?
            }
        };
        if list.is_empty() {
            progress(Progress::Step {
                done: 1,
                total: 1,
                message: tr(
                    "Auf keiner Baudrate antwortet ein Gerät",
                    "No device answers at any baud rate",
                ),
            });
            return Ok(());
        }
        // eingestellte Baudrate zuerst, dort sind meist die meisten Zähler
        let home = self.baud;
        list.sort_by_key(|&b| (b != home, b));
        list.dedup();
        self.scan_bauds(&list, progress, scan)
    }

    pub(super) fn probe_header(
        &mut self,
        address: u8,
    ) -> Result<(Option<u8>, Option<Header>, bool)> {
        self.diag = None;
        match self.request_data(address, false) {
            Ok((primary, frames)) => {
                // brauchbar nur, wenn sich die Antwort decodieren lässt (nicht z.B. "beschäftigt")
                let decoded = frames.first().map(|(ci, data, _)| decode_rsp_ud(*ci, data));
                let usable = matches!(decoded, Some(Ok(_)));
                if !usable {
                    self.note_decode(&frames);
                }
                let header = decoded.and_then(|d| d.ok()?.header);
                Ok((Some(primary), header, usable))
            }
            Err(_) => Ok((None, None, false)),
        }
    }

    pub fn scan_primary(
        &mut self,
        from: u8,
        to: u8,
        retries: u32,
        progress: ProgressFn,
    ) -> Result<()> {
        let total = (to as u32).saturating_sub(from as u32) + 1;
        for (k, a) in (from..=to).enumerate() {
            if !progress(Progress::Step {
                done: k as u32,
                total,
                message: format!("{} {a}", tr("Adresse", "Address")),
            }) {
                return Ok(());
            }
            let mut reply = Reply::None;
            for _ in 0..=retries {
                reply = self.ping(a)?;
                if !matches!(reply, Reply::None) {
                    break;
                }
            }
            let hit = match reply {
                Reply::None => continue,
                Reply::Collision(_) => {
                    let h = ScanHit::from_header(Some(a), None, "collision");
                    self.explain(h)
                }
                Reply::Frame(_) => match self.probe_header(a)? {
                    (_, Some(h), _) => ScanHit::from_header(Some(a), Some(&h), "ok"),
                    (_, None, true) => ScanHit::from_header(Some(a), None, "ok"),
                    (_, None, false) => {
                        let h = ScanHit::from_header(Some(a), None, "no_data");
                        self.explain(h)
                    }
                },
            };
            progress(Progress::Hit(hit));
        }
        progress(Progress::Step {
            done: total,
            total,
            message: tr("Fertig", "Done"),
        });
        Ok(())
    }

    /// Wildcard-Suche über die ID-Stellen (wie libmbus).
    pub fn scan_secondary(&mut self, progress: ProgressFn) -> Result<()> {
        self.scan_secondary_from("", None, progress)
    }

    /// Sekundärscan nur über IDs, die mit `prefix` beginnen (0–7 Ziffern); `resume` = Maske, bei der
    /// ein abgebrochener Scan stand (`Progress::Stopped`): alles davor wird übersprungen.
    pub fn scan_secondary_from(
        &mut self,
        prefix: &str,
        resume: Option<&str>,
        progress: ProgressFn,
    ) -> Result<()> {
        if prefix.len() > 7 || !prefix.chars().all(|c| c.is_ascii_digit()) {
            bail!(
                "{}: {prefix}",
                tr(
                    "Anfang der ID: höchstens 7 Ziffern",
                    "Start of the ID: at most 7 digits"
                )
            );
        }
        let resume: Option<Vec<char>> = resume
            .map(|r| r.to_ascii_uppercase().chars().collect::<Vec<char>>())
            .filter(|r| r.len() == 16 && r.starts_with(&prefix.chars().collect::<Vec<_>>()));
        // Eventuell noch selektierte Zähler freigeben
        self.send(&Frame::snd_nke(ADDR_BROADCAST))?;
        std::thread::sleep(self.inter_byte);
        let mut mask: Vec<char> = format!("{prefix:F<16}").chars().collect();
        let mut cancelled = false;
        let start = prefix.len();
        self.scan_secondary_pos(
            start,
            start,
            resume.as_deref(),
            &mut mask,
            progress,
            &mut cancelled,
        )?;
        if !cancelled {
            progress(Progress::Step {
                done: 10,
                total: 10,
                message: tr("Fertig", "Done"),
            });
        }
        Ok(())
    }

    fn scan_secondary_pos(
        &mut self,
        start: usize,
        pos: usize,
        resume: Option<&[char]>,
        mask: &mut Vec<char>,
        progress: ProgressFn,
        cancelled: &mut bool,
    ) -> Result<()> {
        // auf dem Weg zur Stelle des Abbruchs: erst ab deren Ziffer weitersuchen
        let on_path = resume.filter(|r| r[..pos] == mask[..pos]);
        let first = on_path.and_then(|r| r[pos].to_digit(10)).unwrap_or(0) as u8;
        for digit in first..=9u8 {
            mask[pos] = char::from(b'0' + digit);
            let m: String = mask.iter().collect();
            let done = if pos == start {
                digit as u32
            } else {
                u32::from(mask[start] as u8 - b'0')
            };
            if !progress(Progress::Step {
                done,
                total: 10,
                message: format!("{} {m}", tr("Maske", "Mask")),
            }) {
                *cancelled = true;
                progress(Progress::Stopped {
                    mask: m,
                    baud: None,
                });
                return Ok(());
            }
            let collision = match self.select(&m)? {
                Reply::None => false,
                Reply::Collision(_) => true,
                Reply::Frame(_) => {
                    // ACKs mehrerer Slaves überlagern sich oft fehlerfrei: Daten anfordern prüft das
                    self.diag = None;
                    match self.request_data(ADDR_SELECTED, false) {
                        // ohne lesbaren Datenkopf weitersuchen, bis die ID vollständig ist
                        Ok((_, frames))
                            if pos < 7
                                && frames
                                    .first()
                                    .and_then(|(ci, d, _)| decode_rsp_ud(*ci, d).ok()?.header)
                                    .is_none() =>
                        {
                            true
                        }
                        Ok((primary, frames)) => {
                            let decoded = frames.first().map(|(ci, d, _)| decode_rsp_ud(*ci, d));
                            let status = if matches!(decoded, Some(Ok(_))) {
                                "ok"
                            } else {
                                self.note_decode(&frames);
                                "no_data"
                            };
                            let header = decoded.and_then(|d| d.ok()?.header);
                            let mut hit =
                                ScanHit::from_header(Some(primary), header.as_ref(), status);
                            if hit.secondary_address.is_none() {
                                hit.secondary_address = Some(m.clone());
                            }
                            // ohne lesbaren Datenkopf: ID ist aus der Suche bekannt, sofern vollständig
                            if hit.id.is_none() && !m[..8].contains('F') {
                                hit.id = Some(m[..8].to_string());
                            }
                            let hit = if status == "ok" {
                                hit
                            } else {
                                self.explain(hit)
                            };
                            progress(Progress::Hit(hit));
                            false
                        }
                        // ID vollständig, aber Daten unbrauchbar: Zähler defekt statt Kollision
                        Err(_) if pos == 7 => {
                            let hit = self.explain(ScanHit {
                                secondary_address: Some(m.clone()),
                                id: Some(m[..8].to_string()),
                                ..ScanHit::from_header(None, None, "no_data")
                            });
                            progress(Progress::Hit(hit));
                            false
                        }
                        Err(_) => true,
                    }
                }
            };
            if collision {
                if pos < 7 {
                    self.scan_secondary_pos(start, pos + 1, on_path, mask, progress, cancelled)?;
                    if *cancelled {
                        return Ok(());
                    }
                } else {
                    // gleiche ID bei mehreren Zählern: nur über Hersteller/Version/Medium trennbar
                    progress(Progress::Hit(ScanHit {
                        secondary_address: Some(m.clone()),
                        id: Some(m[..8].to_string()),
                        reason: Some("same_id".into()),
                        ..ScanHit::from_header(None, None, "collision")
                    }));
                }
            }
        }
        mask[pos] = 'F';
        Ok(())
    }
}
