//! A browser session protects both the bundle and the API.

use super::support::{configured, test_server};
use axum::response::IntoResponse;

#[tokio::test]
async fn login_protects_pages_and_api_and_logout_ends_the_session() {
    let mut server = configured(Some("secret"), |settings, _| {
        std::fs::write(
            settings.home.join(".env"),
            format!("QUANTY_SESSION_KEY={}\n", "07".repeat(32)),
        )
        .unwrap();
        settings.session_key = server::Settings::from_sources(&settings.home, |_| None)
            .unwrap()
            .session_key;
    })
    .await;
    let client = server.client();
    let response = client.get(server.url("/")).send().await.unwrap();
    assert_eq!(response.status(), 303);
    assert_eq!(response.headers()["location"], "/login");
    for path in ["/api/facts", "/api/ws", "/content/page.png"] {
        assert_eq!(
            client.get(server.url(path)).send().await.unwrap().status(),
            401
        );
    }
    assert_eq!(
        client
            .get(server.url("/healthz"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "ok"
    );
    let page = client
        .get(server.url("/login"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(page.contains("type=\"password\""));
    assert!(page.contains("action=\"login\""));
    assert!(!page.contains("<script"));
    assert_eq!(
        client
            .post(server.url("/login"))
            .header("origin", "https://attacker.example")
            .header("content-type", "application/x-www-form-urlencoded")
            .body("password=secret")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    for attempt in 1..=5 {
        let response = client
            .post(server.url("/login"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("x-forwarded-for", "192.0.2.1, 127.0.0.1")
            .body("password=wrong")
            .send()
            .await
            .unwrap();
        if attempt < 5 {
            assert!(
                response
                    .text()
                    .await
                    .unwrap()
                    .contains("That is not the password.")
            );
        } else {
            assert_eq!(response.status(), 429);
        }
    }
    assert_eq!(
        client
            .post(server.url("/login"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("x-forwarded-for", "192.0.2.1")
            .body("password=secret")
            .send()
            .await
            .unwrap()
            .status(),
        429
    );
    let response = client
        .post(server.url("/login"))
        .header("content-type", "application/x-www-form-urlencoded")
        .header("x-forwarded-proto", "https")
        .header("host", "quanty.example.com")
        .header("origin", "https://quanty.example.com")
        .body("password=secret")
        .send()
        .await
        .unwrap();
    let set_cookie = response.headers()["set-cookie"].to_str().unwrap();
    for flag in [
        "HttpOnly",
        "SameSite=Lax",
        "Secure",
        "Max-Age=2592000",
        "Path=/",
    ] {
        assert!(set_cookie.contains(flag), "{set_cookie}");
    }
    let cookie = set_cookie.split(';').next().unwrap().to_owned();
    for path in ["/login", "/logout", "/api/upload"] {
        assert_eq!(
            client
                .post(server.url(path))
                .header("cookie", &cookie)
                .header("origin", "https://attacker.example")
                .header("content-type", "application/x-www-form-urlencoded")
                .body("password=secret")
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
    }
    assert_eq!(
        client
            .get(server.url("/api/facts"))
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let response = client
        .get(server.url("/"))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    let has_bundle = env!("QUANTY_UI_BUILT") == "true";
    assert_eq!(response.status(), if has_bundle { 200 } else { 503 });
    assert_eq!(response.headers()["cache-control"], "no-store");
    if response.status() == 503 {
        assert_eq!(
            response.text().await.unwrap(),
            "The UI bundle is not built. Run `trunk build --release` in crates/web-ui, then build quanty again."
        );
    }
    if has_bundle {
        let dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../web-ui/dist");
        let index = client
            .get(server.url("/index.html"))
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap();
        assert_eq!(index.status(), 200);
        assert_eq!(index.headers()["cache-control"], "no-store");
        let mut found = (false, false);
        for file in std::fs::read_dir(dist).unwrap() {
            let name = file.unwrap().file_name().into_string().unwrap();
            let mime = if name.ends_with(".js") {
                found.0 = true;
                "text/javascript; charset=utf-8"
            } else if name.ends_with(".wasm") {
                found.1 = true;
                "application/wasm"
            } else {
                continue;
            };
            let response = client
                .get(server.url(&format!("/{name}")))
                .header("cookie", &cookie)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            assert_eq!(response.headers()["content-type"], mime);
            assert_eq!(
                response.headers()["cache-control"],
                "public, max-age=31536000, immutable"
            );
        }
        assert_eq!(found, (true, true));
    }
    let response = client
        .post(server.url("/logout"))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 303);
    assert!(
        response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    let tampered = format!("{cookie}x");
    assert_eq!(
        client
            .get(server.url("/api/facts"))
            .header("cookie", tampered)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    server.restart().await;
    assert_eq!(
        client
            .get(server.url("/api/facts"))
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let expired = axum_extra::extract::cookie::PrivateCookieJar::new(
        axum_extra::extract::cookie::Key::derive_from(&[7; 32]),
    )
    .add(axum_extra::extract::cookie::Cookie::new(
        "quanty_session",
        "1",
    ))
    .into_response();
    let expired = expired.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    assert_eq!(
        client
            .get(server.url("/api/facts"))
            .header("cookie", expired)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let mut random_key_server = test_server(Some("secret")).await;
    let random_cookie = random_key_server.login(&client).await;
    random_key_server.restart().await;
    assert_eq!(
        client
            .get(random_key_server.url("/api/facts"))
            .header("cookie", random_cookie)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
}
