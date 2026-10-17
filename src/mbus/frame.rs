//! M-Bus Link-Layer (EN 13757-2): Telegramme bauen und parsen.

pub const ACK: u8 = 0xE5;

pub const C_SND_NKE: u8 = 0x40;
pub const C_SND_UD: u8 = 0x53;
pub const C_REQ_UD2: u8 = 0x5B;
pub const FCB: u8 = 0x20;

/// Adresse für Kommunikation über die Sekundäradressierung (Network Layer).
pub const ADDR_SELECTED: u8 = 0xFD;
/// Broadcast, alle Slaves antworten.
pub const ADDR_BROADCAST_REPLY: u8 = 0xFE;
/// Broadcast, kein Slave antwortet.
pub const ADDR_BROADCAST: u8 = 0xFF;

pub const CI_DATA_SEND: u8 = 0x51;
pub const CI_SELECT: u8 = 0x52;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Ack,
    Short { control: u8, address: u8 },
    Long { control: u8, address: u8, ci: u8, data: Vec<u8> },
}

#[derive(Debug, PartialEq, Eq)]
pub enum Parse {
    /// Gültiges Telegramm, Anzahl verbrauchter Bytes.
    Complete(Frame, usize),
    Incomplete,
    Invalid(&'static str),
}

impl Frame {
    pub fn snd_nke(address: u8) -> Self {
        Frame::Short { control: C_SND_NKE, address }
    }

    pub fn req_ud2(address: u8, fcb: bool) -> Self {
        let control = if fcb { C_REQ_UD2 | FCB } else { C_REQ_UD2 };
        Frame::Short { control, address }
    }

    pub fn snd_ud(address: u8, ci: u8, data: Vec<u8>) -> Self {
        Frame::Long { control: C_SND_UD, address, ci, data }
    }

    pub fn encode(&self) -> Vec<u8> {
        match self {
            Frame::Ack => vec![ACK],
            Frame::Short { control, address } => {
                vec![0x10, *control, *address, control.wrapping_add(*address), 0x16]
            }
            Frame::Long { control, address, ci, data } => {
                let l = (data.len() + 3) as u8;
                let mut out = Vec::with_capacity(data.len() + 9);
                out.extend_from_slice(&[0x68, l, l, 0x68, *control, *address, *ci]);
                out.extend_from_slice(data);
                out.push(checksum(&out[4..]));
                out.push(0x16);
                out
            }
        }
    }

    /// RSP_UD: Antwort mit Benutzerdaten (C-Feld 0x08, ggf. mit ACD/DFC-Bits).
    pub fn is_rsp_ud(&self) -> bool {
        matches!(self, Frame::Long { control, .. } if control & 0xCF == 0x08)
    }
}

pub fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |acc, b| acc.wrapping_add(*b))
}

pub fn parse(buf: &[u8]) -> Parse {
    match buf.first() {
        None => Parse::Incomplete,
        Some(&ACK) => Parse::Complete(Frame::Ack, 1),
        Some(&0x10) => {
            if buf.len() < 5 {
                return Parse::Incomplete;
            }
            let (control, address) = (buf[1], buf[2]);
            if buf[3] != control.wrapping_add(address) {
                return Parse::Invalid("Prüfsumme falsch");
            }
            if buf[4] != 0x16 {
                return Parse::Invalid("Stopbyte fehlt");
            }
            Parse::Complete(Frame::Short { control, address }, 5)
        }
        Some(&0x68) => {
            if buf.len() < 4 {
                return Parse::Incomplete;
            }
            let l = buf[1] as usize;
            if buf[2] as usize != l || buf[3] != 0x68 {
                return Parse::Invalid("Header ungültig");
            }
            if l < 3 {
                return Parse::Invalid("Länge ungültig");
            }
            let total = l + 6;
            if buf.len() < total {
                return Parse::Incomplete;
            }
            if checksum(&buf[4..4 + l]) != buf[4 + l] {
                return Parse::Invalid("Prüfsumme falsch");
            }
            if buf[5 + l] != 0x16 {
                return Parse::Invalid("Stopbyte fehlt");
            }
            Parse::Complete(
                Frame::Long {
                    control: buf[4],
                    address: buf[5],
                    ci: buf[6],
                    data: buf[7..4 + l].to_vec(),
                },
                total,
            )
        }
        Some(_) => Parse::Invalid("Startbyte unbekannt"),
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_frame() {
        let f = Frame::req_ud2(5, true);
        assert_eq!(f.encode(), vec![0x10, 0x7B, 0x05, 0x80, 0x16]);
        assert_eq!(parse(&f.encode()), Parse::Complete(f, 5));
    }

    #[test]
    fn long_frame_roundtrip() {
        let f = Frame::snd_ud(1, CI_DATA_SEND, vec![0x01, 0x7A, 0x05]);
        let bytes = f.encode();
        assert_eq!(bytes, vec![0x68, 0x06, 0x06, 0x68, 0x53, 0x01, 0x51, 0x01, 0x7A, 0x05, 0x25, 0x16]);
        assert_eq!(parse(&bytes[..6]), Parse::Incomplete);
        assert_eq!(parse(&bytes), Parse::Complete(f, bytes.len()));
    }

    #[test]
    fn bad_checksum() {
        assert!(matches!(parse(&[0x10, 0x40, 0x01, 0x00, 0x16]), Parse::Invalid(_)));
    }
}
