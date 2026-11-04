use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

mod common;
mod runner_support;
use common::Env;
use runner_support::{gone, turns_started, Fake};

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

async fn connect(port: u16) -> TcpStream {
    for _ in 0..250 {
        if let Ok(stream) = TcpStream::connect(("127.0.0.1", port)).await {
            return stream;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the daemon never listened on {port}");
}

async fn post(port: u16, path: &str, body: &str) -> String {
    let mut stream = connect(port).await;
    let request = format!(
        "POST {path} HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response
}

#[tokio::test]
async fn sigterm_shuts_down_and_ends_running_turns() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let port = free_port();
    let mut daemon = tokio::process::Command::new(env!("CARGO_BIN_EXE_townhall-daemon"))
        .args(["--bind", &format!("127.0.0.1:{port}"), "--repo"])
        .arg(&env.startup)
        .env("TOWNHALL_HOME", env.home.root())
        .env("TOWNHALL_CLAUDE_BIN", fake.bin())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let created = post(port, "/api/conversations", r#"{"text":"SLOW"}"#).await;
    assert!(created.starts_with("HTTP/1.1 201"), "{created}");
    turns_started(&fake, 1).await;
    let claude = fake.pid("pid", 1);
    let pid = daemon.id().unwrap().to_string();
    let sent = std::process::Command::new("kill")
        .args(["-TERM", &pid])
        .status()
        .unwrap();
    assert!(sent.success());
    let exit = tokio::time::timeout(Duration::from_secs(10), daemon.wait())
        .await
        .expect("the daemon exits on SIGTERM")
        .unwrap();
    assert!(
        exit.success(),
        "a clean shutdown, not a signal death: {exit:?}"
    );
    assert!(gone(&claude).await, "the running turn's claude is gone");
}
