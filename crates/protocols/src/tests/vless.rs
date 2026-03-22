use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use titan_config::ProxyConfig;

use crate::{OutboundProxy, Target, VlessProxy};

#[tokio::test]
async fn connects_via_vless_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut prefix = [0_u8; 23];
        socket.read_exact(&mut prefix).await.unwrap();
        assert_eq!(prefix[0], 0x00);
        assert_eq!(prefix[17], 0x00);
        assert_eq!(prefix[18], 0x01);
        assert_eq!(u16::from_be_bytes([prefix[19], prefix[20]]), 443);
        assert_eq!(prefix[21], 0x02);
        assert_eq!(prefix[22] as usize, "example.com".len());

        let mut host = vec![0_u8; prefix[22] as usize];
        socket.read_exact(&mut host).await.unwrap();
        assert_eq!(String::from_utf8(host).unwrap(), "example.com");

        socket.write_all(&[0x00, 0x00]).await.unwrap();
    });

    let config = ProxyConfig {
        name: "vless-outbound".to_string(),
        proxy_type: "vless".to_string(),
        server: addr.ip().to_string(),
        port: addr.port(),
        uuid: Some("123e4567-e89b-12d3-a456-426614174000".to_string()),
        ..Default::default()
    };
    let proxy = VlessProxy::from_config(&config);

    proxy
        .connect_tcp(&Target::new("example.com".to_string(), 443))
        .await
        .unwrap();

    server.await.unwrap();
}
