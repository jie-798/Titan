mod fnv1a;
mod md5;
pub(crate) mod nonce;
pub(crate) mod sha2;
mod stream;
mod typed;

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant, SystemTime};

use aes::cipher::{BlockEncrypt, KeyInit};
use aes::Aes128;
use async_trait::async_trait;
use aws_lc_rs::aead::{
    Aad, BoundKey, OpeningKey, SealingKey, UnboundKey, AES_128_GCM, CHACHA20_POLY1305,
};
use digest::{ExtendableOutput, Update};
use rand::RngCore;
use sha3::Shake128;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use url::Url;

use titan_config::ProxyConfig;

use crate::{BoxedStream, OutboundProxy, Target};

use self::fnv1a::Fnv1aHasher;
use self::md5::{compute_md5, create_chacha_key};
use self::nonce::{SingleUseNonce, VmessNonceSequence};
use self::stream::{ReadHeaderInfo, VmessStream};

const VMESS_UUID_SALT: &[u8] = b"c48619fe-8f02-49e0-b9e9-edf763e17e21";
const COMMAND_TCP: u8 = 0x01;

pub struct VMessProxy {
    name: String,
    server: String,
    port: u16,
    uuid: String,
    cipher: DataCipher,
    tls: bool,
    sni: Option<String>,
    skip_cert_verify: bool,
    alpn: Vec<Vec<u8>>,
    network: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataCipher {
    Aes128Gcm,
    ChaCha20Poly1305,
    None,
}

impl VMessProxy {
    pub fn from_config(config: &ProxyConfig) -> Self {
        Self {
            name: config.name.clone(),
            server: config.server.clone(),
            port: config.port,
            uuid: config.uuid.clone().unwrap_or_default(),
            cipher: DataCipher::from_config(config.cipher.as_deref()),
            tls: config.tls,
            sni: config.sni.clone(),
            skip_cert_verify: config.skip_cert_verify,
            alpn: config
                .alpn
                .clone()
                .unwrap_or_else(|| vec!["h2".to_string(), "http/1.1".to_string()])
                .into_iter()
                .map(|value| value.into_bytes())
                .collect(),
            network: config.network.clone(),
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.uuid.is_empty() {
            anyhow::bail!("VMess proxy {} is missing uuid", self.name);
        }

        if let Some(network) = &self.network {
            if !network.is_empty() && !network.eq_ignore_ascii_case("tcp") {
                anyhow::bail!(
                    "VMess proxy {} only supports tcp network right now, got {}",
                    self.name,
                    network
                );
            }
        }

        Ok(())
    }

    fn create_tls_connector(&self) -> tokio_rustls::TlsConnector {
        let mut config = if self.skip_cert_verify {
            rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(SkipCertVerifier))
                .with_no_client_auth()
        } else {
            let mut root_store = rustls::RootCertStore::empty();
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            rustls::ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth()
        };
        config.alpn_protocols = self.alpn.clone();
        tokio_rustls::TlsConnector::from(Arc::new(config))
    }

    async fn connect_transport(&self) -> anyhow::Result<WrappedVmessTransport> {
        let addr = format!("{}:{}", self.server, self.port);
        let stream = TcpStream::connect(&addr).await?;

        if !self.tls {
            return Ok(WrappedVmessTransport::Plain(stream));
        }

        let server_name_str = self.sni.as_deref().unwrap_or(&self.server).to_owned();
        let server_name = rustls::pki_types::ServerName::try_from(server_name_str.as_str())
            .map_err(|_| anyhow::anyhow!("invalid VMess server name: {}", server_name_str))?
            .to_owned();
        let connector = self.create_tls_connector();
        let stream = connector.connect(server_name, stream).await?;
        Ok(WrappedVmessTransport::Tls(stream))
    }

    async fn write_request(
        &self,
        transport: &mut WrappedVmessTransport,
        target: &Target,
    ) -> anyhow::Result<BoxedStream> {
        let user_id = uuid::Uuid::parse_str(&self.uuid)?;
        let instruction_key = instruction_key(user_id.as_bytes());
        let auth_key = sha2::kdf(&instruction_key, &[b"AES Auth ID Encryption"]);
        let aead_cipher = Aes128::new((&auth_key[0..16]).into());
        let cert_hash = build_auth_id(&aead_cipher);

        let mut header = [0u8; 316];
        header[0] = 1;
        rand::rng().fill_bytes(&mut header[1..34]);

        let data_encryption_iv = &header[1..17];
        let data_encryption_key = &header[17..33];
        let response_authentication_v = header[33];

        let mut response_header_iv = [0u8; 16];
        response_header_iv.copy_from_slice(&sha2::compute_sha256(data_encryption_iv)[0..16]);
        let mut response_header_key = [0u8; 16];
        response_header_key.copy_from_slice(&sha2::compute_sha256(data_encryption_key)[0..16]);

        let mut request_hasher = Shake128::default();
        request_hasher.update(data_encryption_iv);
        let request_reader = request_hasher.finalize_xof();

        let mut response_hasher = Shake128::default();
        response_hasher.update(&response_header_iv);
        let response_reader = response_hasher.finalize_xof();

        let (encryption_method, encryption_keys) = self.build_data_keys(
            data_encryption_key,
            data_encryption_iv,
            &response_header_key,
            &response_header_iv,
        )?;

        header[34] = 0x01 | 0x04;
        let margin_len = rand::random::<u8>() & 0x0f;
        header[35] = (margin_len << 4) | encryption_method;
        header[36] = 0;
        header[37] = COMMAND_TCP;
        header[38] = (target.port >> 8) as u8;
        header[39] = (target.port & 0xff) as u8;

        let mut cursor = 40 + encode_target(&mut header[40..], target)?;
        if margin_len > 0 {
            rand::rng().fill_bytes(&mut header[cursor..cursor + margin_len as usize]);
            cursor += margin_len as usize;
        }

        let mut fnv = Fnv1aHasher::new();
        fnv.write(&header[..cursor]);
        header[cursor..cursor + 4].copy_from_slice(&fnv.finish().to_be_bytes());
        cursor += 4;

        let mut nonce = [0u8; 8];
        rand::rng().fill_bytes(&mut nonce);

        let mut encrypted_length = [0u8; 18];
        encrypted_length[0..2].copy_from_slice(&(cursor as u16).to_be_bytes());
        let header_len_key = sha2::kdf(
            &instruction_key,
            &[b"VMess Header AEAD Key_Length", &cert_hash, &nonce],
        );
        let header_len_nonce = sha2::kdf(
            &instruction_key,
            &[b"VMess Header AEAD Nonce_Length", &cert_hash, &nonce],
        );
        let unbound = UnboundKey::new(&AES_128_GCM, &header_len_key[0..16])?;
        let mut sealing = SealingKey::new(unbound, SingleUseNonce::new(&header_len_nonce[0..12]));
        let tag = sealing
            .seal_in_place_separate_tag(Aad::from(&cert_hash), &mut encrypted_length[0..2])?;
        encrypted_length[2..].copy_from_slice(tag.as_ref());

        let header_key =
            sha2::kdf(&instruction_key, &[b"VMess Header AEAD Key", &cert_hash, &nonce]);
        let header_nonce =
            sha2::kdf(&instruction_key, &[b"VMess Header AEAD Nonce", &cert_hash, &nonce]);
        let unbound = UnboundKey::new(&AES_128_GCM, &header_key[0..16])?;
        let mut sealing = SealingKey::new(unbound, SingleUseNonce::new(&header_nonce[0..12]));
        let mut encrypted_header = header[..cursor].to_vec();
        let tag = sealing
            .seal_in_place_separate_tag(Aad::from(&cert_hash), &mut encrypted_header)?;
        encrypted_header.extend_from_slice(tag.as_ref());

        transport.write_all(&cert_hash).await?;
        transport.write_all(&encrypted_length).await?;
        transport.write_all(&nonce).await?;
        transport.write_all(&encrypted_header).await?;
        transport.flush().await?;

        Ok(Box::new(VmessStream::new(
            transport.take(),
            encryption_keys,
            Some(response_reader),
            Some(request_reader),
            Some(ReadHeaderInfo {
                response_header_key,
                response_header_iv,
                response_authentication_v,
            }),
        )))
    }

    fn build_data_keys(
        &self,
        data_encryption_key: &[u8],
        data_encryption_iv: &[u8],
        response_header_key: &[u8; 16],
        response_header_iv: &[u8; 16],
    ) -> anyhow::Result<(
        u8,
        Option<(OpeningKey<VmessNonceSequence>, SealingKey<VmessNonceSequence>)>,
    )> {
        match self.cipher {
            DataCipher::Aes128Gcm => {
                let opening = OpeningKey::new(
                    UnboundKey::new(&AES_128_GCM, response_header_key)?,
                    VmessNonceSequence::new(response_header_iv),
                );
                let sealing = SealingKey::new(
                    UnboundKey::new(&AES_128_GCM, data_encryption_key)?,
                    VmessNonceSequence::new(data_encryption_iv),
                );
                Ok((3, Some((opening, sealing))))
            }
            DataCipher::ChaCha20Poly1305 => {
                let opening = OpeningKey::new(
                    UnboundKey::new(&CHACHA20_POLY1305, &create_chacha_key(response_header_key))?,
                    VmessNonceSequence::new(response_header_iv),
                );
                let sealing = SealingKey::new(
                    UnboundKey::new(&CHACHA20_POLY1305, &create_chacha_key(data_encryption_key))?,
                    VmessNonceSequence::new(data_encryption_iv),
                );
                Ok((4, Some((opening, sealing))))
            }
            DataCipher::None => Ok((5, None)),
        }
    }
}

#[async_trait]
impl OutboundProxy for VMessProxy {
    fn name(&self) -> &str {
        &self.name
    }

    async fn connect_tcp(&self, target: &Target) -> anyhow::Result<BoxedStream> {
        self.validate()?;
        let mut transport = self.connect_transport().await?;
        self.write_request(&mut transport, target).await
    }

    async fn delay_test(&self, url: &str, timeout: Duration) -> anyhow::Result<Duration> {
        let parsed = Url::parse(url)?;
        let host = parsed
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("test URL is missing a host"))?;
        let port = parsed
            .port_or_known_default()
            .ok_or_else(|| anyhow::anyhow!("test URL is missing a port"))?;
        let target = Target::new(host.to_string(), port);

        let start = Instant::now();
        tokio::time::timeout(timeout, self.connect_tcp(&target)).await??;
        Ok(start.elapsed())
    }
}

impl DataCipher {
    fn from_config(cipher: Option<&str>) -> Self {
        match cipher.unwrap_or("auto").to_ascii_lowercase().as_str() {
            "" | "auto" | "any" | "aes-128-gcm" => Self::Aes128Gcm,
            "chacha20-poly1305" | "chacha20-ietf-poly1305" => Self::ChaCha20Poly1305,
            "none" => Self::None,
            other => {
                tracing::warn!("unsupported VMess cipher {}, fallback to aes-128-gcm", other);
                Self::Aes128Gcm
            }
        }
    }
}

enum WrappedVmessTransport {
    Plain(TcpStream),
    Tls(TlsStream<TcpStream>),
    Taken,
}

impl WrappedVmessTransport {
    fn take(&mut self) -> Self {
        std::mem::replace(self, Self::Taken)
    }
}

impl AsyncRead for WrappedVmessTransport {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_read(cx, buf),
            Self::Tls(stream) => Pin::new(stream).poll_read(cx, buf),
            Self::Taken => Poll::Ready(Err(std::io::Error::other("transport already taken"))),
        }
    }
}

impl AsyncWrite for WrappedVmessTransport {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_write(cx, buf),
            Self::Tls(stream) => Pin::new(stream).poll_write(cx, buf),
            Self::Taken => Poll::Ready(Err(std::io::Error::other("transport already taken"))),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_flush(cx),
            Self::Tls(stream) => Pin::new(stream).poll_flush(cx),
            Self::Taken => Poll::Ready(Err(std::io::Error::other("transport already taken"))),
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Tls(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Taken => Poll::Ready(Err(std::io::Error::other("transport already taken"))),
        }
    }
}

pub(crate) fn instruction_key(uuid_bytes: &[u8]) -> [u8; 16] {
    let mut value = Vec::with_capacity(uuid_bytes.len() + VMESS_UUID_SALT.len());
    value.extend_from_slice(uuid_bytes);
    value.extend_from_slice(VMESS_UUID_SALT);
    compute_md5(&value)
}

fn build_auth_id(cipher: &Aes128) -> [u8; 16] {
    let mut auth_id = [0u8; 16];
    let now = SystemTime::UNIX_EPOCH
        .elapsed()
        .unwrap_or_default()
        .as_secs()
        .to_be_bytes();
    auth_id[0..8].copy_from_slice(&now);
    rand::rng().fill_bytes(&mut auth_id[8..12]);
    let checksum = crc32c::crc32c(&auth_id[0..12]).to_be_bytes();
    auth_id[12..16].copy_from_slice(&checksum);
    cipher.encrypt_block((&mut auth_id).into());
    auth_id
}

fn encode_target(buf: &mut [u8], target: &Target) -> anyhow::Result<usize> {
    if let Ok(ipv4) = target.host.parse::<std::net::Ipv4Addr>() {
        buf[0] = 0x01;
        buf[1..5].copy_from_slice(&ipv4.octets());
        return Ok(5);
    }

    if let Ok(ipv6) = target.host.parse::<std::net::Ipv6Addr>() {
        buf[0] = 0x03;
        buf[1..17].copy_from_slice(&ipv6.octets());
        return Ok(17);
    }

    if target.host.len() > u8::MAX as usize {
        anyhow::bail!("VMess target hostname is too long");
    }

    buf[0] = 0x02;
    buf[1] = target.host.len() as u8;
    buf[2..2 + target.host.len()].copy_from_slice(target.host.as_bytes());
    Ok(2 + target.host.len())
}

#[derive(Debug)]
struct SkipCertVerifier;

impl rustls::client::danger::ServerCertVerifier for SkipCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ED448,
        ]
    }
}
