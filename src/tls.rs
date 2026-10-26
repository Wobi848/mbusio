//! TLS-Einstellungen für die eigenen Clients (MQTT, E-Mail): öffentliche Zertifizierungsstellen,
//! eigene CA oder ohne Prüfung (selbst signiert, nur im geschützten Netz)

use std::sync::Arc;

use anyhow::{bail, Context, Result};

use crate::i18n::tr;

#[derive(Debug)]
struct NoVerify(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for NoVerify {
    fn verify_server_cert(
        &self,
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// TLS ohne Prüfung (`insecure`), mit eigener CA (PEM) oder den üblichen öffentlichen
pub fn config(insecure: bool, ca: &str) -> Result<rustls::ClientConfig> {
    use rustls::pki_types::pem::PemObject;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()?;
    if insecure {
        return Ok(builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerify(provider)))
            .with_no_client_auth());
    }
    let mut roots = rustls::RootCertStore::empty();
    if ca.trim().is_empty() {
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    } else {
        for cert in rustls::pki_types::CertificateDer::pem_slice_iter(ca.as_bytes()) {
            roots.add(cert.context(tr("CA-Zertifikat unlesbar", "CA certificate unreadable"))?)?;
        }
        if roots.is_empty() {
            bail!(tr(
                "Kein Zertifikat im CA-Feld (PEM erwartet)",
                "No certificate in the CA field (PEM expected)"
            ));
        }
    }
    Ok(builder.with_root_certificates(roots).with_no_client_auth())
}
