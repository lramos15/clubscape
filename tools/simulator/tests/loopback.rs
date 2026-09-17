use std::{
    io::{ErrorKind, Write},
    net::TcpListener,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[test]
fn synthetic_client_never_routes_local_credentials_through_environment_proxies() {
    let origin = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
    origin.set_nonblocking(true).unwrap();
    proxy.set_nonblocking(true).unwrap();
    let proxy_url = format!("http://{}", proxy.local_addr().unwrap());
    let mut child = Command::new(env!("CARGO_BIN_EXE_clubscape-sim"))
        .args([
            "account-lifecycle",
            "--url",
            &format!("http://{}", origin.local_addr().unwrap()),
        ])
        .envs([
            ("HTTP_PROXY", &proxy_url),
            ("http_proxy", &proxy_url),
            ("HTTPS_PROXY", &proxy_url),
            ("https_proxy", &proxy_url),
            ("ALL_PROXY", &proxy_url),
            ("all_proxy", &proxy_url),
            ("NO_PROXY", &String::new()),
            ("no_proxy", &String::new()),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut reached_origin = false;
    let mut reached_proxy = false;
    let mut connection_error = None;
    while Instant::now() < deadline {
        for (listener, reached) in [(&origin, &mut reached_origin), (&proxy, &mut reached_proxy)] {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    *reached = true;
                    if let Err(error) = stream
                        .set_write_timeout(Some(Duration::from_secs(1)))
                        .and_then(|()| {
                            stream.write_all(
                                b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            )
                        })
                    {
                        connection_error = Some(error);
                    }
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                Err(error) => connection_error = Some(error),
            }
        }
        if child.try_wait().unwrap().is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let timed_out = child.try_wait().unwrap().is_none();
    if timed_out {
        child.kill().unwrap();
    }
    let status = child.wait().unwrap();
    assert!(!timed_out, "the bounded local-client check did not finish");
    assert!(connection_error.is_none(), "{connection_error:?}");
    assert!(
        !status.success(),
        "the fixture deliberately returns an error"
    );
    assert!(
        reached_origin,
        "the request must reach the actual local server"
    );
    assert!(
        !reached_proxy,
        "local account traffic must not leave through a proxy"
    );
}
