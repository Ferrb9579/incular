use incular_devtools::{AgentHandle, discovery::list_sessions, session::SessionConfig, spawn};
use std::{sync::Arc, time::Duration};
use tokio::io::AsyncReadExt;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::StatusCode};

fn agent() -> (AgentHandle, String) {
    let name = format!(
        "security-{}",
        incular_devtools::session::generate_token().unwrap()
    );
    let handle = spawn(
        SessionConfig {
            app_name: name.clone(),
        },
        tokio::runtime::Handle::current(),
        Arc::new(|| {}),
    )
    .unwrap();
    let (record, _) = list_sessions()
        .into_iter()
        .find(|(r, _)| r.app_name == name)
        .unwrap();
    (handle, format!("127.0.0.1:{}", record.port))
}

#[tokio::test]
async fn idle_tcp_upgrade_expires_and_the_next_client_can_connect() {
    let (handle, address) = agent();
    let mut idle = tokio::net::TcpStream::connect(&address).await.unwrap();
    let mut byte = [0u8];
    let count = tokio::time::timeout(Duration::from_secs(10), idle.read(&mut byte))
        .await
        .expect("idle upgrade must expire")
        .unwrap();
    assert_eq!(count, 0);
    let (mut websocket, _) = tokio_tungstenite::connect_async(format!("ws://{address}"))
        .await
        .unwrap();
    websocket.close(None).await.unwrap();
    handle.stop();
    tokio::time::timeout(Duration::from_secs(2), handle.stopped())
        .await
        .unwrap();
}

#[tokio::test]
async fn stop_cancels_a_partial_http_upgrade() {
    use tokio::io::AsyncWriteExt;
    let (handle, address) = agent();
    let mut idle = tokio::net::TcpStream::connect(&address).await.unwrap();
    idle.write_all(b"GET / HTTP/1.1\r\n").await.unwrap();
    // Give the accept task a chance to enter the partial HTTP handshake.
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.stop();
    tokio::time::timeout(Duration::from_secs(2), handle.stopped())
        .await
        .expect("shutdown must not wait for the upgrade deadline");
}

#[tokio::test]
async fn browser_origin_is_rejected_before_authentication() {
    let (handle, address) = agent();
    let mut request = format!("ws://{address}").into_client_request().unwrap();
    request
        .headers_mut()
        .insert("origin", "https://untrusted.example".parse().unwrap());
    let error = tokio_tungstenite::connect_async(request).await.unwrap_err();
    match error {
        tokio_tungstenite::tungstenite::Error::Http(response) => {
            assert_eq!(response.status(), StatusCode::FORBIDDEN)
        }
        other => panic!("expected HTTP rejection, received {other}"),
    }
    handle.stop();
    tokio::time::timeout(Duration::from_secs(2), handle.stopped())
        .await
        .unwrap();
}
