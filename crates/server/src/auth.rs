//! One encrypted browser session protects the library without introducing accounts.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::{
    Form,
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar, SameSite};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::Settings;

const SESSION: &str = "quanty_session";
const LIFETIME: u64 = 30 * 24 * 60 * 60;
const WINDOW: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub(crate) struct Auth {
    settings: Settings,
    failures: Arc<Mutex<HashMap<String, Attempts>>>,
}
struct Attempts {
    count: u32,
    started: Instant,
    blocked_until: Option<Instant>,
}

impl Auth {
    pub(crate) fn new(settings: Settings) -> Self {
        Self {
            settings,
            failures: Arc::default(),
        }
    }
    fn jar(&self, headers: &HeaderMap) -> PrivateCookieJar {
        PrivateCookieJar::from_headers(headers, self.settings.session_key.clone())
    }
    fn scheme(&self, headers: &HeaderMap, uri: &axum::http::Uri) -> Option<&'static str> {
        if self.settings.password.is_none() {
            return Some("http");
        }
        let mut forwarded = headers.get_all("x-forwarded-proto").iter();
        let proto = forwarded
            .next()
            .map(|value| value.to_str())
            .transpose()
            .ok()?;
        if forwarded.next().is_some() || proto.is_some_and(|proto| proto.contains(',')) {
            return None;
        }
        Some(
            if uri.scheme_str() == Some("https")
                || proto.is_some_and(|proto| proto.eq_ignore_ascii_case("https"))
            {
                "https"
            } else {
                "http"
            },
        )
    }

    fn accepts_origin(&self, request: &Request) -> bool {
        let Some(scheme) = self.scheme(request.headers(), request.uri()) else {
            return false;
        };
        let mut origins = request.headers().get_all("origin").iter();
        let Some(origin) = origins.next() else {
            return true;
        };
        if origins.next().is_some() {
            return false;
        }
        let Some((origin_scheme, name)) = origin
            .to_str()
            .ok()
            .and_then(|origin| origin.split_once("://"))
        else {
            return false;
        };
        if !origin_scheme.eq_ignore_ascii_case(scheme) {
            return false;
        }
        let Some(origin) = parse_authority(name) else {
            return false;
        };
        let Some(host) = request_host(request.headers()) else {
            return false;
        };
        let default_port = if scheme == "https" { 443 } else { 80 };
        same_host(origin.host(), host.host())
            && origin.port_u16().unwrap_or(default_port) == host.port_u16().unwrap_or(default_port)
    }

    fn is_signed_in(&self, headers: &HeaderMap) -> bool {
        self.settings.password.is_none()
            || self
                .jar(headers)
                .get(SESSION)
                .and_then(|cookie| cookie.value().parse::<u64>().ok())
                .is_some_and(|expires| expires > now())
    }
}

fn matches_secret(given: &str, expected: &str) -> bool {
    bool::from(Sha256::digest(given.as_bytes()).ct_eq(&Sha256::digest(expected.as_bytes())))
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) async fn protect(State(auth): State<Auth>, request: Request, next: Next) -> Response {
    if auth.settings.password.is_none() && !has_local_host(request.headers(), auth.settings.listen)
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    if auth.is_signed_in(request.headers()) {
        return next.run(request).await;
    }
    if request.uri().path().starts_with("/api/") || request.uri().path().starts_with("/content/") {
        StatusCode::UNAUTHORIZED.into_response()
    } else {
        Redirect::to("/login").into_response()
    }
}

fn request_host(headers: &HeaderMap) -> Option<axum::http::uri::Authority> {
    let mut values = headers.get_all("host").iter();
    let host = values.next()?.to_str().ok()?;
    if values.next().is_some() || host.contains('@') {
        return None;
    }
    parse_authority(host)
}

fn parse_authority(value: &str) -> Option<axum::http::uri::Authority> {
    if value.contains(['@', '/', '?', '#', ',']) {
        return None;
    }
    let authority: axum::http::uri::Authority = value.parse().ok()?;
    let suffix = authority.as_str().strip_prefix(authority.host())?;
    if !suffix.is_empty() {
        let port = suffix.strip_prefix(':')?;
        if port.is_empty()
            || !port.bytes().all(|byte| byte.is_ascii_digit())
            || port.parse::<u16>().is_err()
        {
            return None;
        }
    }
    Some(authority)
}

fn same_host(first: &str, second: &str) -> bool {
    let ip = |host: &str| {
        host.trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<std::net::IpAddr>()
    };
    match (ip(first), ip(second)) {
        (Ok(first), Ok(second)) => first == second,
        _ => first.eq_ignore_ascii_case(second),
    }
}

pub(crate) async fn check_origin(
    State(auth): State<Auth>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    let needs_origin_check = path == "/api/ws"
        || (request.method() == axum::http::Method::POST
            && matches!(path, "/login" | "/logout" | "/api/upload"));
    if needs_origin_check
        && ((!auth.accepts_origin(&request))
            || (auth.settings.password.is_none()
                && !has_local_host(request.headers(), auth.settings.listen)))
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(request).await
}

fn has_local_host(headers: &HeaderMap, listen: SocketAddr) -> bool {
    let Some(host) = request_host(headers) else {
        return false;
    };
    let name = host.host();
    let ip = name.trim_start_matches('[').trim_end_matches(']');
    let is_loopback = name.eq_ignore_ascii_case("localhost")
        || ip
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    is_loopback && host.port_u16().unwrap_or(80) == listen.port()
}

pub(crate) async fn login_page() -> Html<String> {
    page("")
}
fn page(message: &str) -> Html<String> {
    Html(format!(
        r#"<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>quanty — sign in</title><style>body{{background:#f8f5ee;color:#322f28;font:18px Georgia,serif;max-width:24rem;margin:15vh auto;padding:1rem}}input,button{{box-sizing:border-box;width:100%;padding:.7rem;margin:.5rem 0;font:inherit}}button{{background:#d8cbb2;border:1px solid #94846a;cursor:pointer}}</style><h1>quanty</h1><form method="post" action="/login"><label for="password">Password</label><input id="password" name="password" type="password" autocomplete="current-password" required autofocus><button>Sign in</button><p>{message}</p></form></html>"#
    ))
}
#[derive(serde::Deserialize)]
pub(crate) struct Login {
    password: String,
}

pub(crate) async fn login(
    State(auth): State<Auth>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    Form(form): Form<Login>,
) -> Response {
    if auth.settings.password.is_none() {
        return Redirect::to("/").into_response();
    }
    let address = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| peer.ip().to_string());
    let current = Instant::now();
    let mut failures = auth.failures.lock().unwrap_or_else(|e| e.into_inner());
    failures.retain(|_, a| {
        a.blocked_until.is_some_and(|end| end > current)
            || current.duration_since(a.started) < WINDOW
    });
    if failures
        .get(&address)
        .is_some_and(|a| a.blocked_until.is_some_and(|until| until > current))
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", "60")],
            page("Too many attempts. Try again in a minute."),
        )
            .into_response();
    }
    if !matches_secret(
        &form.password,
        auth.settings.password.as_deref().unwrap_or_default(),
    ) {
        let attempts = failures.entry(address).or_insert(Attempts {
            count: 0,
            started: current,
            blocked_until: None,
        });
        attempts.count += 1;
        if attempts.count >= 5 {
            attempts.blocked_until = Some(current + WINDOW);
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", "60")],
                page("Too many attempts. Try again in a minute."),
            )
                .into_response();
        }
        return page("That is not the password.").into_response();
    }
    failures.remove(&address);
    drop(failures);
    let Some(scheme) = auth.scheme(&headers, &uri) else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let cookie = Cookie::build((SESSION, (now() + LIFETIME).to_string()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(scheme == "https")
        .max_age(time::Duration::seconds(LIFETIME as i64))
        .build();
    (auth.jar(&headers).add(cookie), Redirect::to("/")).into_response()
}

pub(crate) async fn logout(State(auth): State<Auth>, headers: HeaderMap) -> impl IntoResponse {
    (
        auth.jar(&headers).remove(Cookie::build(SESSION).path("/")),
        Redirect::to("/login"),
    )
}

pub(crate) async fn bearer(State(auth): State<Auth>, request: Request, next: Next) -> Response {
    let Some(token) = auth.settings.mcp_token.as_deref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let given = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    if !given.is_some_and(|given| matches_secret(given, token)) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(request).await
}
