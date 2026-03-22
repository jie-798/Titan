use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

use crate::{inbound::InboundServer, ProxyEngine};

#[tokio::test]
async fn forwards_http_proxy_requests_to_direct_target() {
    let (origin_addr, origin_task, request_rx) = spawn_origin_server().await;

    let config = titan_config::Config {
        rules: vec!["MATCH,DIRECT".to_string()],
        ..Default::default()
    };
    let engine = Arc::new(ProxyEngine::new(config).await.unwrap());
    engine.start().await.unwrap();

    let (inbound_addr, inbound_task) = spawn_inbound_server(engine.clone()).await;

    let mut client = TcpStream::connect(inbound_addr).await.unwrap();
    let request = format!(
        "GET http://{}/hello?name=titan HTTP/1.1\r\nHost: {}\r\nProxy-Connection: keep-alive\r\nConnection: close\r\n\r\n",
        origin_addr, origin_addr
    );
    client.write_all(request.as_bytes()).await.unwrap();

    let response = read_until_eof(&mut client).await;
    let response_text = String::from_utf8_lossy(&response);
    assert!(response_text.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response_text.contains("X-Origin: titan-test"));
    assert!(response_text.ends_with("hello from origin"));

    let forwarded = tokio::time::timeout(Duration::from_secs(3), request_rx)
        .await
        .unwrap()
        .unwrap();
    let forwarded_text = String::from_utf8_lossy(&forwarded);
    assert!(forwarded_text.starts_with("GET /hello?name=titan HTTP/1.1\r\n"));
    assert!(forwarded_text.contains(&format!("Host: {}\r\n", origin_addr)));
    assert!(!forwarded_text.to_ascii_lowercase().contains("proxy-connection:"));

    inbound_task.abort();
    origin_task.abort();
}

#[tokio::test]
async fn returns_bad_gateway_for_rejected_http_proxy_request() {
    let config = titan_config::Config {
        rules: vec![
            "DOMAIN,blocked.test,REJECT".to_string(),
            "MATCH,DIRECT".to_string(),
        ],
        ..Default::default()
    };
    let engine = Arc::new(ProxyEngine::new(config).await.unwrap());
    engine.start().await.unwrap();

    let (inbound_addr, inbound_task) = spawn_inbound_server(engine).await;

    let mut client = TcpStream::connect(inbound_addr).await.unwrap();
    client
        .write_all(
            b"GET http://blocked.test/ HTTP/1.1\r\nHost: blocked.test\r\nConnection: close\r\n\r\n",
        )
        .await
        .unwrap();

    let response = read_until_eof(&mut client).await;
    let response_text = String::from_utf8_lossy(&response);
    assert!(response_text.starts_with("HTTP/1.1 502 Bad Gateway\r\n"));

    inbound_task.abort();
}

async fn spawn_origin_server(
) -> (
    SocketAddr,
    tokio::task::JoinHandle<()>,
    oneshot::Receiver<Vec<u8>>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (request_tx, request_rx) = oneshot::channel();

    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_http_message(&mut stream).await;
        let _ = request_tx.send(request);
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 17\r\nConnection: close\r\nX-Origin: titan-test\r\n\r\nhello from origin",
            )
            .await
            .unwrap();
    });

    (addr, task, request_rx)
}

async fn spawn_inbound_server(
    engine: Arc<ProxyEngine>,
) -> (SocketAddr, tokio::task::JoinHandle<anyhow::Result<()>>) {
    let bind_addr = reserve_local_addr().await;
    let server = InboundServer::new(bind_addr, engine);
    let task = tokio::spawn(async move { server.start().await });

    tokio::time::sleep(Duration::from_millis(50)).await;

    (bind_addr, task)
}

async fn reserve_local_addr() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    addr
}

async fn read_http_message(stream: &mut TcpStream) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut chunk = [0_u8; 1024];

    loop {
        let read = stream.read(&mut chunk).await.unwrap();
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..read]);
        if buf.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }

    buf
}

async fn read_until_eof(stream: &mut TcpStream) -> Vec<u8> {
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.unwrap();
    buf
}
