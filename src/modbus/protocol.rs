//! Modbus TCP: Anfragen aus dem Datenstrom lösen und beantworten (Funktionen 03 und 04).

/// Fehlercodes nach Modbus
const ILLEGAL_FUNCTION: u8 = 0x01;
const ILLEGAL_ADDRESS: u8 = 0x02;
const ILLEGAL_VALUE: u8 = 0x03;
/// Höchstens so viele Register je Anfrage (Modbus-Vorgabe)
const MAX_REGISTERS: u16 = 125;

/// Was am Anfang des Puffers steht
#[derive(Debug, PartialEq)]
pub enum Next {
    /// noch nicht vollständig
    Need,
    /// kein Modbus TCP (Protokoll-ID nicht 0 oder Länge unmöglich): Puffer verwerfen
    Garbage,
    /// vollständige Anfrage dieser Länge (MBAP-Kopf mit PDU)
    Request(usize),
}

/// Kopf: Transaktion (2), Protokoll (2, immer 0), Länge (2, ab Unit-ID), Unit-ID (1)
pub fn next_request(buf: &[u8]) -> Next {
    if buf.len() < 7 {
        return Next::Need;
    }
    let protocol = u16::from_be_bytes([buf[2], buf[3]]);
    let len = u16::from_be_bytes([buf[4], buf[5]]) as usize;
    if protocol != 0 || !(2..=254).contains(&len) {
        return Next::Garbage;
    }
    if buf.len() < 6 + len {
        return Next::Need;
    }
    Next::Request(6 + len)
}

/// Antwort auf eine vollständige Anfrage, mit demselben Kopf
pub fn answer(req: &[u8], regs: &[u16]) -> Vec<u8> {
    let function = req[7];
    let pdu = match function {
        0x03 | 0x04 if req.len() >= 12 => read(function, &req[8..12], regs),
        0x03 | 0x04 => Err(ILLEGAL_VALUE),
        _ => Err(ILLEGAL_FUNCTION),
    }
    .unwrap_or_else(|code| vec![function | 0x80, code]);
    let mut out = Vec::with_capacity(7 + pdu.len());
    out.extend_from_slice(&req[0..4]);
    out.extend_from_slice(&(pdu.len() as u16 + 1).to_be_bytes());
    out.push(req[6]);
    out.extend_from_slice(&pdu);
    out
}

/// Register lesen: Start (2), Anzahl (2)
fn read(function: u8, args: &[u8], regs: &[u16]) -> Result<Vec<u8>, u8> {
    let start = u16::from_be_bytes([args[0], args[1]]) as usize;
    let count = u16::from_be_bytes([args[2], args[3]]);
    if count == 0 || count > MAX_REGISTERS {
        return Err(ILLEGAL_VALUE);
    }
    let end = start + count as usize;
    if end > regs.len() {
        return Err(ILLEGAL_ADDRESS);
    }
    let mut pdu = vec![function, (count * 2) as u8];
    for r in &regs[start..end] {
        pdu.extend_from_slice(&r.to_be_bytes());
    }
    Ok(pdu)
}
