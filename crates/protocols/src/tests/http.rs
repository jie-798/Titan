use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use titan_config::ProxyConfig;

use crate::{HttpProxy, OutboundProxy, Target};

#[tokio::test]
async fn connects_via_http_connect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut chunk = [0_u8; 256];

        loop {
            let read = socket.read(&mut chunk).await.unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }

        let request_text = String::from_utf8_lossy(&request);
        assert!(request_text.contains("CONNECT example.com:443 HTTP/1.1"));
        assert!(request_text.contains("Host: example.com:443"));

        socket
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await
            .unwrap();
    });

    let config = ProxyConfig {
        name: "http-outbound".to_string(),
        proxy_type: "http".to_string(),
        server: addr.ip().to_string(),
        port: addr.port(),
        ..Default::default()
    };
    let proxy = HttpProxy::from_config(&config);

    proxy
        .connect_tcp(&Target::new("example.com".to_string(), 443))
        .await
        .unwrap();

    server.await.unwrap();
}
