use aes::cipher::{BlockDecrypt, KeyInit};
use aes::Aes128;
use aws_lc_rs::aead::{Aad, BoundKey, OpeningKey, SealingKey, UnboundKey, AES_128_GCM};
use digest::{ExtendableOutput, Update, XofReader};
use sha3::Shake128;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use titan_config::ProxyConfig;

use crate::vmess::nonce::{SingleUseNonce, VmessNonceSequence};
use crate::vmess::sha2;
use crate::vmess::instruction_key;
use crate::{OutboundProxy, Target, VMessProxy};

const TAG_LEN: usize = 16;
const COMMAND_TCP: u8 = 0x01;

#[tokio::test]
async fn connects_via_vmess_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let uuid = uuid::Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").unwrap();

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        let instruction_key = instruction_key(uuid.as_bytes());
        let auth_key = sha2::kdf(&instruction_key, &[b"AES Auth ID Encryption"]);
        let cipher = Aes128::new((&auth_key[0..16]).into());

        let mut cert_hash = [0u8; 16];
        socket.read_exact(&mut cert_hash).await.unwrap();
        let mut auth_id = cert_hash;
        cipher.decrypt_block((&mut auth_id).into());
        assert_eq!(
            u32::from_be_bytes(auth_id[12..16].try_into().unwrap()),
            crc32c::crc32c(&auth_id[0..12])
        );

        let mut encrypted_length = [0u8; 18];
        socket.read_exact(&mut encrypted_length).await.unwrap();
        let mut nonce = [0u8; 8];
        socket.read_exact(&mut nonce).await.unwrap();

        let header_len_key = sha2::kdf(
            &instruction_key,
            &[b"VMess Header AEAD Key_Length", &cert_hash, &nonce],
        );
        let header_len_nonce = sha2::kdf(
            &instruction_key,
            &[b"VMess Header AEAD Nonce_Length", &cert_hash, &nonce],
        );
        let unbound = UnboundKey::new(&AES_128_GCM, &header_len_key[0..16]).unwrap();
        let mut opening =
            OpeningKey::new(unbound, SingleUseNonce::new(&header_len_nonce[0..12]));
        let len_bytes = opening
            .open_in_place(Aad::from(&cert_hash), &mut encrypted_length)
            .unwrap();
        let header_len = u16::from_be_bytes([len_bytes[0], len_bytes[1]]) as usize;

        let mut encrypted_header = vec![0u8; header_len + TAG_LEN];
        socket.read_exact(&mut encrypted_header).await.unwrap();
        let header_key = sha2::kdf(&instruction_key, &[b"VMess Header AEAD Key", &cert_hash, &nonce]);
        let header_nonce =
            sha2::kdf(&instruction_key, &[b"VMess Header AEAD Nonce", &cert_hash, &nonce]);
        let unbound = UnboundKey::new(&AES_128_GCM, &header_key[0..16]).unwrap();
        let mut opening =
            OpeningKey::new(unbound, SingleUseNonce::new(&header_nonce[0..12]));
        let header = opening
            .open_in_place(Aad::from(&cert_hash), &mut encrypted_header)
            .unwrap();

        assert_eq!(header[0], 1);
        assert_eq!(header[37], COMMAND_TCP);
        assert_eq!(u16::from_be_bytes([header[38], header[39]]), 443);
        assert_eq!(header[40], 0x02);
        assert_eq!(header[41] as usize, "example.com".len());
        assert_eq!(
            std::str::from_utf8(&header[42..42 + "example.com".len()]).unwrap(),
            "example.com"
        );

        let data_encryption_iv = &header[1..17];
        let data_encryption_key = &header[17..33];
        let response_authentication_v = header[33];

        let mut response_header_iv = [0u8; 16];
        response_header_iv.copy_from_slice(&sha2::compute_sha256(data_encryption_iv)[0..16]);
        let mut response_header_key = [0u8; 16];
        response_header_key.copy_from_slice(&sha2::compute_sha256(data_encryption_key)[0..16]);

        let mut encrypted_response_length = [0u8; 18];
        encrypted_response_length[0..2].copy_from_slice(&4u16.to_be_bytes());
        let len_key = sha2::kdf(&response_header_key, &[b"AEAD Resp Header Len Key"]);
        let len_nonce = sha2::kdf(&response_header_iv, &[b"AEAD Resp Header Len IV"]);
        let unbound = UnboundKey::new(&AES_128_GCM, &len_key[0..16]).unwrap();
        let mut sealing = SealingKey::new(unbound, SingleUseNonce::new(&len_nonce[0..12]));
        let tag = sealing
            .seal_in_place_separate_tag(Aad::empty(), &mut encrypted_response_length[0..2])
            .unwrap();
        encrypted_response_length[2..].copy_from_slice(tag.as_ref());

        let mut encrypted_response = [0u8; 4 + TAG_LEN];
        encrypted_response[0] = response_authentication_v;
        let content_key = sha2::kdf(&response_header_key, &[b"AEAD Resp Header Key"]);
        let content_nonce = sha2::kdf(&response_header_iv, &[b"AEAD Resp Header IV"]);
        let unbound = UnboundKey::new(&AES_128_GCM, &content_key[0..16]).unwrap();
        let mut sealing = SealingKey::new(unbound, SingleUseNonce::new(&content_nonce[0..12]));
        let tag = sealing
            .seal_in_place_separate_tag(Aad::empty(), &mut encrypted_response[0..4])
            .unwrap();
        encrypted_response[4..].copy_from_slice(tag.as_ref());

        socket.write_all(&encrypted_response_length).await.unwrap();
        socket.write_all(&encrypted_response).await.unwrap();

        let mut response_hasher = Shake128::default();
        response_hasher.update(&response_header_iv);
        let mut response_reader = response_hasher.finalize_xof();
        let mut mask_bytes = [0u8; 2];
        response_reader.read(&mut mask_bytes);
        let mask = u16::from_be_bytes(mask_bytes);

        let mut payload = b"ok".to_vec();
        let unbound = UnboundKey::new(&AES_128_GCM, &response_header_key).unwrap();
        let mut sealing = SealingKey::new(unbound, VmessNonceSequence::new(&response_header_iv));
        let tag = sealing
            .seal_in_place_separate_tag(Aad::empty(), &mut payload)
            .unwrap();
        payload.extend_from_slice(tag.as_ref());

        let framed_len = (payload.len() as u16) ^ mask;
        socket.write_all(&framed_len.to_be_bytes()).await.unwrap();
        socket.write_all(&payload).await.unwrap();
    });

    let config = ProxyConfig {
        name: "vmess-outbound".to_string(),
        proxy_type: "vmess".to_string(),
        server: addr.ip().to_string(),
        port: addr.port(),
        uuid: Some(uuid.to_string()),
        cipher: Some("auto".to_string()),
        ..Default::default()
    };
    let proxy = VMessProxy::from_config(&config);

    let mut stream = proxy
        .connect_tcp(&Target::new("example.com".to_string(), 443))
        .await
        .unwrap();

    let mut data = [0u8; 2];
    stream.read_exact(&mut data).await.unwrap();
    assert_eq!(&data, b"ok");

    server.await.unwrap();
}
