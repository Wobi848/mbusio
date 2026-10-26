//! Nachricht im Internet-Format: Kopf, Text und Anhänge als multipart/mixed, alles in Base64
//! (dann braucht es keine Rücksicht auf Zeilenlänge, Umlaute oder Punkte am Zeilenanfang).

pub struct Attachment {
    pub name: String,
    pub mime: String,
    pub data: Vec<u8>,
}

pub struct Message {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub text: String,
    pub attachments: Vec<Attachment>,
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Base64 in Zeilen zu 76 Zeichen
fn base64_lines(data: &[u8]) -> String {
    let b = base64(data);
    let mut out = String::new();
    for line in b.as_bytes().chunks(76) {
        out.push_str(std::str::from_utf8(line).unwrap_or_default());
        out.push_str("\r\n");
    }
    out
}

/// Text für eine Kopfzeile: ohne Zeilenumbrüche, mit Umlauten als „encoded word“
fn header_text(s: &str) -> String {
    let s: String = s.chars().filter(|c| !c.is_control()).collect();
    if s.is_ascii() {
        s
    } else {
        format!("=?UTF-8?B?{}?=", base64(s.as_bytes()))
    }
}

/// Dateiname als Parameter: ASCII-Ersatz und UTF-8 nach RFC 2231
fn file_param(name: &str) -> String {
    let ascii: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || ".-_".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded: String = name
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b".-_".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    format!("filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}

impl Message {
    /// Ganze Nachricht mit CRLF; `date` im Format von RFC 5322, `id` eindeutig
    pub fn render(&self, date: &str, id: &str) -> String {
        let boundary = format!("mbusio-{id}");
        let domain = self.from.split('@').nth(1).unwrap_or("mbusio");
        let mut out = format!(
            "From: {}\r\nTo: {}\r\nSubject: {}\r\nDate: {date}\r\nMessage-ID: <{id}@{domain}>\r\n\
             MIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"{boundary}\"\r\n\r\n",
            self.from,
            self.to.join(", "),
            header_text(&self.subject),
        );
        out.push_str(&format!(
            "--{boundary}\r\nContent-Type: text/plain; charset=utf-8\r\n\
             Content-Transfer-Encoding: base64\r\n\r\n{}",
            base64_lines(self.text.as_bytes())
        ));
        for a in &self.attachments {
            out.push_str(&format!(
                "--{boundary}\r\nContent-Type: {}\r\nContent-Disposition: attachment; {}\r\n\
                 Content-Transfer-Encoding: base64\r\n\r\n{}",
                a.mime,
                file_param(&a.name),
                base64_lines(&a.data)
            ));
        }
        out.push_str(&format!("--{boundary}--\r\n"));
        out
    }
}
