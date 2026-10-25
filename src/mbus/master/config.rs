//! Zähler einstellen: Primäradresse, Baudrate, Application Reset; freie Adresse suchen

use anyhow::{bail, Result};

use super::*;

impl Master {
    pub fn set_primary(&mut self, target: &Target, new: u8) -> Result<()> {
        if new > 250 {
            bail!(tr(
                "Neue Primäradresse muss 0..250 sein",
                "New primary address must be 0..250"
            ));
        }
        // Neue Adresse muss frei sein, sonst stehen danach zwei Zähler darauf
        if !matches!(self.ping(new)?, Reply::None) {
            if let Target::Secondary(mask) = target {
                // Steht der Zähler selbst schon dort, ist nichts zu tun
                if let (_, Some(h), _) = self.probe_header(new)? {
                    if mask_matches(mask, &h.secondary_address) {
                        return Ok(());
                    }
                }
            } else if *target == Target::Primary(new) {
                return Ok(());
            }
            bail!(
                "{}: {new}",
                tr(
                    "Neue Primäradresse ist schon belegt",
                    "New primary address is already in use"
                )
            );
        }
        self.command(
            target,
            CI_DATA_SEND,
            vec![0x01, 0x7A, new],
            &tr(
                "Zähler hat die Adressänderung nicht quittiert",
                "Meter did not acknowledge the address change",
            ),
        )?;
        match self.ping(new)? {
            Reply::Frame(Frame::Ack) => Ok(()),
            _ => bail!(
                "{} {new}",
                tr(
                    "Adresse gesetzt, aber Zähler antwortet nicht auf neuer Adresse",
                    "Address set, but meter does not answer on new address"
                )
            ),
        }
    }

    /// Bricht ab, wenn auf der Primäradresse keiner oder mehrere Zähler antworten.
    fn ensure_single(&mut self, a: u8) -> Result<()> {
        match self.ping(a)? {
            Reply::Frame(_) => Ok(()),
            Reply::None => bail!(
                "{} {a}",
                tr("Keine Antwort von Adresse", "No reply from address")
            ),
            Reply::Collision(_) => bail!(
                "{} {a}: {}",
                tr("Mehrere Zähler auf Adresse", "Several meters on address"),
                tr(
                    "bitte über Sekundäradresse oder ID ändern",
                    "please use the secondary address or ID"
                )
            ),
        }
    }

    /// Befehl an genau einen Zähler senden und auf die Quittung warten. Bei einer Primäradresse
    /// vorher prüfen, dass dort nur ein Zähler antwortet: sonst würden alle umgestellt.
    fn command(&mut self, target: &Target, ci: u8, data: Vec<u8>, no_ack: &str) -> Result<()> {
        if let Target::Primary(a) = target {
            self.ensure_single(*a)?;
        }
        let address = self.address(target)?;
        self.expect_ack(&Frame::snd_ud(address, ci, data), no_ack)
    }

    fn expect_ack(&mut self, f: &Frame, no_reply: &str) -> Result<()> {
        match self.transact(f)? {
            Reply::Frame(Frame::Ack) => Ok(()),
            Reply::None => bail!("{no_reply}"),
            other => bail!(
                "{}: {other:?}",
                tr("Unerwartete Antwort", "Unexpected reply")
            ),
        }
    }

    /// Antwortet das Ziel (Primär: SND_NKE, Sekundär: Selektion)?
    fn answers(&mut self, target: &Target) -> Result<bool> {
        Ok(match target {
            Target::Primary(a) => !matches!(self.ping(*a)?, Reply::None),
            Target::Secondary(mask) => matches!(self.select(mask)?, Reply::Frame(Frame::Ack)),
        })
    }

    /// Baudrate am Zähler umstellen (CI B8..BF) und mit der neuen Baudrate kontrollieren.
    /// Der Master bleibt danach auf seiner bisherigen Baudrate.
    pub fn switch_baud(&mut self, target: &Target, new: u32) -> Result<()> {
        let ci = baud_ci(new)?;
        let old = self.baud;
        // TCP-Gateways können nicht umschalten; vorher prüfen statt hinterher
        self.link.set_baud(old)?;
        if new == old {
            return Ok(());
        }
        self.command(
            target,
            ci,
            Vec::new(),
            &tr(
                "Zähler hat den Baudratenwechsel nicht quittiert",
                "Meter did not acknowledge the baud rate change",
            ),
        )?;
        // Zähler wechselt erst nach dem ACK
        std::thread::sleep(self.inter_byte);
        self.set_baud(new)?;
        let ok = self.answers(target);
        self.set_baud(old)?;
        if !ok? {
            bail!(
                "{} {new} Bd",
                tr(
                    "Zähler hat quittiert, antwortet aber nicht mit",
                    "Meter acknowledged, but does not answer at"
                )
            );
        }
        Ok(())
    }

    /// Probiert alle Standard-Baudraten durch und liefert die, auf denen das Ziel antwortet.
    pub fn detect_baud(&mut self, target: &Target) -> Result<Vec<u32>> {
        let old = self.baud;
        self.link.set_baud(old)?;
        let result = self.try_bauds(target);
        self.set_baud(old)?;
        result
    }

    fn try_bauds(&mut self, target: &Target) -> Result<Vec<u32>> {
        let mut found = Vec::new();
        for baud in BAUD_RATES {
            self.set_baud(baud)?;
            // einmal wiederholen, manche Zähler verpassen das erste Telegramm nach Pause
            if self.answers(target)? || self.answers(target)? {
                found.push(baud);
            }
        }
        Ok(found)
    }

    /// Erste Primäradresse ab `from`, auf der niemand antwortet (zweimal gefragt).
    /// `skip`: schon bekannte belegte Adressen, die gar nicht erst abgefragt werden.
    pub fn free_primary(&mut self, from: u8, skip: &[u8]) -> Result<u8> {
        for a in from.max(1)..=250 {
            if skip.contains(&a) {
                continue;
            }
            if matches!(self.ping(a)?, Reply::None) && matches!(self.ping(a)?, Reply::None) {
                return Ok(a);
            }
        }
        bail!(tr(
            "Keine freie Primäradresse gefunden",
            "No free primary address found"
        ))
    }

    /// Application Reset (CI 50), optional mit Subcode.
    pub fn application_reset(&mut self, target: &Target, subcode: Option<u8>) -> Result<()> {
        self.command(
            target,
            CI_APP_RESET,
            subcode.into_iter().collect(),
            &tr(
                "Zähler hat den Reset nicht quittiert",
                "Meter did not acknowledge the reset",
            ),
        )
    }
}

/// CI-Feld für den Baudratenwechsel (EN 13757-3): B8 = 300 … BF = 38400
fn baud_ci(baud: u32) -> Result<u8> {
    check_baud([baud])?;
    let k = BAUD_RATES
        .iter()
        .position(|&b| b == baud)
        .unwrap_or_default();
    Ok(0xB8 + k as u8)
}
