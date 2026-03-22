use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use titan_config::ProxyConfig;

use crate::{OutboundProxy, Socks5Proxy, Target};

#[tokio::test]
async fn connects_via_socks5() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        let version = socket.read_u8().await.unwrap();
        let method_count = socket.read_u8().await.unwrap() as usize;
        let mut methods = vec![0_u8; method_count];
        socket.read_exact(&mut methods).await.unwrap();
        assert_eq!(version, 0x05);
        assert_eq!(methods, vec![0x00]);
        socket.write_all(&[0x05, 0x00]).await.unwrap();

        let version = socket.read_u8().await.unwrap();
        let command = socket.read_u8().await.unwrap();
        let reserved = socket.read_u8().await.unwrap();
        let atyp = socket.read_u8().await.unwrap();
        assert_eq!(version, 0x05);
        assert_eq!(command, 0x01);
        assert_eq!(reserved, 0x00);
        assert_eq!(atyp, 0x03);

        let len = socket.read_u8().await.unwrap() as usize;
        let mut host = vec![0_u8; len];
        socket.read_exact(&mut host).await.unwrap();
        let port = socket.read_u16().await.unwrap();

        assert_eq!(String::from_utf8(host).unwrap(), "example.com");
        assert_eq!(port, 443);

        socket
            .write_all(&[0x05, 0x00, 0x00, 0x01, 127, 0, 0, 1, 0x1F, 0x90])
            .await
            .unwrap();
    });

    let config = ProxyConfig {
        name: "socks5-outbound".to_string(),
        proxy_type: "socks5".to_string(),
        server: addr.ip().to_string(),
        port: addr.port(),
        ..Default::default()
    };
    let proxy = Socks5Proxy::from_config(&config);

    proxy
        .connect_tcp(&Target::new("example.com".to_string(), 443))
        .await
        .unwrap();

    server.await.unwrap();
}
