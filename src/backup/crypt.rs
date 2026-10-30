//! Sicherung mit einem Passwort verschlüsseln, Datei zu Datei in Stücken: die Datenbank kann
//! Hunderte MB gross sein, der Pi hat wenig Arbeitsspeicher. Schlüssel aus PBKDF2-SHA256, je Stück
//! AES-256-GCM.
//!
//! Format: `MBUSIOV2` | Runden (u32) | Salt (16) | Nonce-Anfang (7) | Stücke. Ein Stück ist
//! höchstens 1 MiB Klartext plus 16 Byte Prüfsumme. Die Nonce zählt die Stücke und markiert das
//! letzte: vertauschte, fehlende oder abgeschnittene Stücke fallen beim Entschlüsseln auf.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::num::NonZeroU32;
use std::path::Path;

use anyhow::{anyhow, Result};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::pbkdf2;
use ring::rand::{SecureRandom, SystemRandom};

use crate::i18n::tr;

const MAGIC: &[u8] = b"MBUSIOV2";
const ROUNDS: u32 = 200_000;
const SALT_LEN: usize = 16;
/// Anfang der Nonce (zufällig je Datei); dahinter Nummer des Stücks (4) und „letztes“ (1)
const PREFIX_LEN: usize = NONCE_LEN - 5;
const HEAD_LEN: usize = MAGIC.len() + 4 + SALT_LEN + PREFIX_LEN;
/// Klartext je Stück
const CHUNK: usize = 1 << 20;
const TAG_LEN: usize = 16;

fn key_from(password: &str, salt: &[u8], rounds: u32) -> LessSafeKey {
    let mut bytes = [0u8; 32];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(rounds).unwrap_or(NonZeroU32::MIN),
        salt,
        password.as_bytes(),
        &mut bytes,
    );
    // AES-256 verlangt einen 32-Byte-Schlüssel, genau was pbkdf2::derive oben liefert
    LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &bytes).expect("32-Byte-Schlüssel"))
}

fn nonce(prefix: &[u8], n: u32, last: bool) -> Nonce {
    let mut bytes = [0u8; NONCE_LEN];
    bytes[..PREFIX_LEN].copy_from_slice(prefix);
    bytes[PREFIX_LEN..PREFIX_LEN + 4].copy_from_slice(&n.to_be_bytes());
    bytes[NONCE_LEN - 1] = last as u8;
    Nonce::assume_unique_for_key(bytes)
}

/// Erkennt an den ersten Bytes, ob eine Datei mit [`encrypt_file`] verschlüsselt wurde
pub fn is_encrypted(head: &[u8]) -> bool {
    head.starts_with(MAGIC)
}

pub fn is_encrypted_file(path: &Path) -> Result<bool> {
    let mut head = Vec::with_capacity(MAGIC.len());
    File::open(path)?
        .take(MAGIC.len() as u64)
        .read_to_end(&mut head)?;
    Ok(is_encrypted(&head))
}

/// `src` mit einem Passwort nach `dst` verschlüsseln; eigenes Salz und eigene Nonce je Datei
pub fn encrypt_file(password: &str, src: &Path, dst: &Path) -> Result<()> {
    let rng = SystemRandom::new();
    let mut salt = [0u8; SALT_LEN];
    let mut prefix = [0u8; PREFIX_LEN];
    rng.fill(&mut salt)
        .and_then(|()| rng.fill(&mut prefix))
        .map_err(|_| {
            anyhow!(tr(
                "Keine Zufallszahl verfügbar",
                "no random number available"
            ))
        })?;
    let key = key_from(password, &salt, ROUNDS);
    let mut out = BufWriter::new(File::create(dst)?);
    out.write_all(MAGIC)?;
    out.write_all(&ROUNDS.to_be_bytes())?;
    out.write_all(&salt)?;
    out.write_all(&prefix)?;
    for (n, chunk) in Chunks::new(File::open(src)?, CHUNK).enumerate() {
        let (mut data, last) = chunk?;
        key.seal_in_place_append_tag(nonce(&prefix, n as u32, last), Aad::empty(), &mut data)
            .map_err(|_| anyhow!(tr("Verschlüsseln fehlgeschlagen", "encryption failed")))?;
        out.write_all(&data)?;
    }
    out.flush()?;
    Ok(())
}

/// `src` mit dem Passwort nach `dst` entschlüsseln. Falsches Passwort und veränderte oder
/// abgeschnittene Daten ergeben denselben Fehler; `dst` bleibt dann nicht liegen.
pub fn decrypt_file(password: &str, src: &Path, dst: &Path) -> Result<()> {
    let result = decrypt_into(password, src, dst);
    if result.is_err() {
        std::fs::remove_file(dst).ok();
    }
    result
}

fn decrypt_into(password: &str, src: &Path, dst: &Path) -> Result<()> {
    let not_ours = || {
        anyhow!(tr(
            "Keine verschlüsselte Sicherung von mbusio",
            "not an encrypted mbusio backup"
        ))
    };
    let mut input = BufReader::new(File::open(src)?);
    let mut head = [0u8; HEAD_LEN];
    input.read_exact(&mut head).map_err(|_| not_ours())?;
    if !is_encrypted(&head) {
        return Err(not_ours());
    }
    let at = MAGIC.len();
    let rounds = u32::from_be_bytes(head[at..at + 4].try_into()?);
    let salt = &head[at + 4..at + 4 + SALT_LEN];
    let prefix = &head[at + 4 + SALT_LEN..];
    let key = key_from(password, salt, rounds);
    let mut out = BufWriter::new(File::create(dst)?);
    for (n, chunk) in Chunks::new(input, CHUNK + TAG_LEN).enumerate() {
        let (mut data, last) = chunk?;
        let plain = key
            .open_in_place(nonce(prefix, n as u32, last), Aad::empty(), &mut data)
            .map_err(|_| {
                anyhow!(tr(
                    "Passwort falsch oder Sicherung beschädigt",
                    "wrong password or backup damaged"
                ))
            })?;
        out.write_all(plain)?;
    }
    out.flush()?;
    Ok(())
}

/// Stücke fester Grösse aus einer Datei, mit dem Hinweis, ob es das letzte ist (dafür wird eins
/// im Voraus gelesen). Eine leere Datei ergibt ein leeres letztes Stück.
struct Chunks<R> {
    input: R,
    size: usize,
    ahead: Option<Vec<u8>>,
    done: bool,
}

impl<R: Read> Chunks<R> {
    fn new(input: R, size: usize) -> Self {
        Chunks {
            input,
            size,
            ahead: None,
            done: false,
        }
    }

    fn fill(&mut self) -> std::io::Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(self.size);
        (&mut self.input)
            .take(self.size as u64)
            .read_to_end(&mut buf)?;
        Ok(buf)
    }

    fn step(&mut self) -> std::io::Result<(Vec<u8>, bool)> {
        let current = match self.ahead.take() {
            Some(c) => c,
            None => self.fill()?,
        };
        let next = if current.len() == self.size {
            self.fill()?
        } else {
            Vec::new()
        };
        let last = next.is_empty();
        self.done = last;
        if !last {
            self.ahead = Some(next);
        }
        Ok((current, last))
    }
}

impl<R: Read> Iterator for Chunks<R> {
    type Item = std::io::Result<(Vec<u8>, bool)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let item = self.step();
        if item.is_err() {
            self.done = true;
        }
        Some(item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mbusio-crypt-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// mehrere Stücke, letztes kurz; plus Grenzfälle leer und genau ein volles Stück
    fn samples() -> Vec<Vec<u8>> {
        vec![
            (0..CHUNK * 2 + 123).map(|i| (i % 251) as u8).collect(),
            Vec::new(),
            vec![7; CHUNK],
        ]
    }

    #[test]
    fn round_trip_in_chunks() {
        let d = dir("rund");
        for (i, data) in samples().into_iter().enumerate() {
            let (plain, sealed, back) = (d.join("p"), d.join("s"), d.join("b"));
            std::fs::write(&plain, &data).unwrap();
            encrypt_file("Grüße", &plain, &sealed).unwrap();
            assert!(is_encrypted_file(&sealed).unwrap());
            decrypt_file("Grüße", &sealed, &back).unwrap();
            assert_eq!(std::fs::read(&back).unwrap(), data, "Probe {i}");
        }
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn wrong_password_cut_or_swapped_chunks_fail() {
        let d = dir("falsch");
        let (plain, sealed, back) = (d.join("p"), d.join("s"), d.join("b"));
        std::fs::write(&plain, &samples()[0]).unwrap();
        encrypt_file("geheim", &plain, &sealed).unwrap();
        assert!(decrypt_file("falsch", &sealed, &back).is_err());
        assert!(!back.exists(), "nichts Halbes liegen lassen");
        let full = std::fs::read(&sealed).unwrap();
        let piece = CHUNK + TAG_LEN;
        // genau an einer Stückgrenze abgeschnitten: das letzte gelesene war nicht als letztes markiert
        std::fs::write(&sealed, &full[..HEAD_LEN + piece]).unwrap();
        assert!(decrypt_file("geheim", &sealed, &back).is_err());
        // erste zwei Stücke vertauscht
        let mut swapped = full[..HEAD_LEN].to_vec();
        swapped.extend_from_slice(&full[HEAD_LEN + piece..HEAD_LEN + 2 * piece]);
        swapped.extend_from_slice(&full[HEAD_LEN..HEAD_LEN + piece]);
        swapped.extend_from_slice(&full[HEAD_LEN + 2 * piece..]);
        std::fs::write(&sealed, &swapped).unwrap();
        assert!(decrypt_file("geheim", &sealed, &back).is_err());
        // keine Sicherung von mbusio
        std::fs::write(&sealed, b"SQLite format 3\0").unwrap();
        assert!(decrypt_file("geheim", &sealed, &back).is_err());
        std::fs::remove_dir_all(d).unwrap();
    }
}
