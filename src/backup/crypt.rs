//! Sicherung mit einem Passwort verschlüsseln: Schlüssel aus PBKDF2-SHA256, Daten mit
//! AES-256-GCM. Format: `MBUSIOV1` | Runden (u32) | Salt (16) | Nonce (12) | Daten+Tag

use std::num::NonZeroU32;

use anyhow::{anyhow, bail, Result};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::pbkdf2;
use ring::rand::{SecureRandom, SystemRandom};

use crate::i18n::tr;

const MAGIC: &[u8] = b"MBUSIOV1";
const ROUNDS: u32 = 200_000;
const SALT_LEN: usize = 16;
const HEAD_LEN: usize = MAGIC.len() + 4 + SALT_LEN + NONCE_LEN;

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

/// Erkennt an den ersten Bytes, ob eine Datei mit [`encrypt`] verschlüsselt wurde
pub fn is_encrypted(data: &[u8]) -> bool {
    data.starts_with(MAGIC)
}

/// Mit einem Passwort verschlüsseln; eigenes Salz und eigene Nonce je Aufruf
pub fn encrypt(password: &str, data: &[u8]) -> Result<Vec<u8>> {
    let rng = SystemRandom::new();
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    rng.fill(&mut salt)
        .and_then(|()| rng.fill(&mut nonce))
        .map_err(|_| {
            anyhow!(tr(
                "Keine Zufallszahl verfügbar",
                "no random number available"
            ))
        })?;
    let key = key_from(password, &salt, ROUNDS);
    let mut sealed = data.to_vec();
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::empty(),
        &mut sealed,
    )
    .map_err(|_| anyhow!(tr("Verschlüsseln fehlgeschlagen", "encryption failed")))?;
    let mut out = Vec::with_capacity(HEAD_LEN + sealed.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&ROUNDS.to_be_bytes());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// Mit einem Passwort entschlüsseln; falsches Passwort und veränderte Daten ergeben beide
/// denselben Fehler (AES-GCM prüft die Unversehrtheit mit)
pub fn decrypt(password: &str, data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < HEAD_LEN || !is_encrypted(data) {
        bail!(tr(
            "Keine verschlüsselte Sicherung von mbusio",
            "not an encrypted mbusio backup"
        ));
    }
    let rounds = u32::from_be_bytes(data[MAGIC.len()..MAGIC.len() + 4].try_into().unwrap());
    let salt = &data[MAGIC.len() + 4..MAGIC.len() + 4 + SALT_LEN];
    let nonce: [u8; NONCE_LEN] = data[MAGIC.len() + 4 + SALT_LEN..HEAD_LEN]
        .try_into()
        .unwrap();
    let key = key_from(password, salt, rounds);
    let mut sealed = data[HEAD_LEN..].to_vec();
    let plain = key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::empty(),
            &mut sealed,
        )
        .map_err(|_| {
            anyhow!(tr(
                "Passwort falsch oder Sicherung beschädigt",
                "wrong password or backup damaged"
            ))
        })?;
    Ok(plain.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let sealed = encrypt("geheim", b"Hallo Sicherung").unwrap();
        assert!(is_encrypted(&sealed));
        assert_eq!(decrypt("geheim", &sealed).unwrap(), b"Hallo Sicherung");
    }

    #[test]
    fn wrong_password_and_tampering_fail() {
        let mut sealed = encrypt("geheim", b"Daten").unwrap();
        assert!(decrypt("falsch", &sealed).is_err());
        let last = sealed.len() - 1;
        sealed[last] ^= 1;
        assert!(decrypt("geheim", &sealed).is_err());
    }

    #[test]
    fn unknown_format_is_rejected() {
        assert!(!is_encrypted(b"SQLite format 3\0"));
        assert!(decrypt("geheim", b"SQLite format 3\0").is_err());
    }
}
