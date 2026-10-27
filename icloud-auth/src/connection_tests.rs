use super::gsa_client;
use reqwest::cookie::{CookieStore, Jar};
use std::{io::{Read, Write}, net::TcpListener, sync::{Arc, mpsc}, thread, time::{Duration, Instant}};

#[tokio::test]
async fn gsa_requests_use_fresh_connections_and_preserve_cookies() {
    // Local-only fixture: no Apple account, anisette provider, or real network
    // authentication. A keep-alive response must not pool the first socket.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut sockets = Vec::new();
        while sockets.len() < 2 {
            match listener.accept() {
                Ok((mut socket, _)) => {
                    socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                    let mut request = Vec::new();
                    let mut byte = [0];
                    while !request.ends_with(b"\r\n\r\n") {
                        socket.read_exact(&mut byte).unwrap();
                        request.push(byte[0]);
                    }
                    tx.send(String::from_utf8(request).unwrap()).unwrap();
                    socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: keep-alive\r\nSet-Cookie: gsa_fixture=kept; Path=/\r\n\r\nok").unwrap();
                    sockets.push(socket);
                },
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "second request reused the first connection");
                    thread::sleep(Duration::from_millis(5));
                },
                Err(error) => panic!("fixture failed: {error}"),
            }
        }
    });
    let cookies = Arc::new(Jar::default());
    let client = gsa_client(cookies.clone()).unwrap();
    for _ in 0..2 {
        assert_eq!(client.get(&url).send().await.unwrap().text().await.unwrap(), "ok");
    }
    let first = rx.recv_timeout(Duration::from_secs(1)).unwrap().to_ascii_lowercase();
    let second = rx.recv_timeout(Duration::from_secs(1)).unwrap().to_ascii_lowercase();
    assert!(first.contains("connection: close\r\n"));
    assert!(second.contains("connection: close\r\n"));
    assert!(second.contains("cookie: gsa_fixture=kept\r\n"));
    assert!(cookies.cookies(&url.parse().unwrap()).is_some());
    server.join().unwrap();
}
