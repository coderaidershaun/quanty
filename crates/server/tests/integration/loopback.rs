//! Startup refuses an exposed library without a password and enters the requested home.

use server::Settings;
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

#[tokio::test]
async fn listen_flags_override_environment_and_home_settings_and_loopback_needs_no_password() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join(".env"),
        "QUANTY_LISTEN=0.0.0.0:12345\nQUANTY_UPLOAD_LIMIT_MB=5\n",
    )
    .unwrap();
    let settings = Settings::from_sources(home.path(), |name| {
        (name == "QUANTY_LISTEN").then(|| "127.0.0.1:45678".into())
    })
    .unwrap();
    assert_eq!(settings.listen.to_string(), "127.0.0.1:45678");
    assert_eq!(settings.upload_limit, 5 * 1024 * 1024);
    let failed = Command::new(env!("CARGO_BIN_EXE_quanty"))
        .env_clear()
        .arg("--home")
        .arg(home.path())
        .args(["--fixture", "black-scholes"])
        .output()
        .await
        .unwrap();
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("QUANTY_PASSWORD is required"));
    let mut child = Command::new(env!("CARGO_BIN_EXE_quanty"))
        .env_clear()
        .env("QUANTY_LISTEN", "0.0.0.0:56789")
        .arg("--home")
        .arg(home.path())
        .args(["--fixture", "black-scholes", "--listen", "127.0.0.1:0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let line = tokio::time::timeout(Duration::from_secs(20), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let url = line.strip_prefix("quanty is listening on ").unwrap();
    let client = reqwest::Client::new();
    assert_eq!(
        client
            .get(format!("{url}/api/facts"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .post(format!("{url}/api/upload"))
            .header("x-file-name", "home.pdf")
            .body("%PDF-1.7\n")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert!(home.path().join("data/uploads").exists());
    child.kill().await.unwrap();
    child.wait().await.unwrap();
    let listing = Command::new(env!("CARGO_BIN_EXE_quanty"))
        .env_clear()
        .args(["--fixture", "list"])
        .output()
        .await
        .unwrap();
    assert!(listing.status.success());
    assert!(String::from_utf8_lossy(&listing.stdout).contains("black-scholes"));
    std::fs::write(
        home.path().join(".env"),
        "QUANTY_PASSWORD=\"audit-secret-marker\n",
    )
    .unwrap();
    let malformed = Command::new(env!("CARGO_BIN_EXE_quanty"))
        .env_clear()
        .arg("--home")
        .arg(home.path())
        .args(["--fixture", "black-scholes"])
        .output()
        .await
        .unwrap();
    assert!(!malformed.status.success());
    let stderr = String::from_utf8_lossy(&malformed.stderr);
    assert!(stderr.contains("could not parse settings"));
    assert!(!stderr.contains("audit-secret-marker"));
    assert!(!String::from_utf8_lossy(&malformed.stdout).contains("audit-secret-marker"));
}
