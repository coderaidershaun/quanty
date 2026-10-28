//! Fetches a PDF that an agent named by URL: only from an allowed origin, never from a host name
//! that leads into a private network, with no redirect, and no more bytes than the size limit.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use reqwest::redirect::Policy;
use reqwest::{Client, Response, Url, header::LOCATION};

use super::{PDF_START, PdfIngestError};
use crate::services::Services;

/// How long the server waits to connect, so that a host which never answers cannot hold the
/// ingest slot.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the whole fetch may take, from the first byte sent to the last byte read.
const TOTAL_TIMEOUT: Duration = Duration::from_secs(120);

/// The bytes at `url`, when the URL is on this machine or on one of the `allowed` origins, the
/// host answers with a PDF, and the PDF is no bigger than `max_pdf_bytes`. Nothing is connected to
/// before the origin is checked. A host name is looked up once through `services`, every address it
/// has must be a public one, and the connection goes to those addresses only. A redirect is
/// refused and not followed.
///
/// # Errors
/// - [`PdfIngestError::NotAUrl`], [`PdfIngestError::PdfUrlScheme`] and
///   [`PdfIngestError::PdfUrlOrigin`] for a URL that is refused before any connection
/// - [`PdfIngestError::PdfUrlResolve`] when the host name has no address
/// - [`PdfIngestError::PdfUrlInternalAddress`] when the host name has an address inside a private
///   network or on this machine
/// - [`PdfIngestError::PdfUrlRedirect`] and [`PdfIngestError::PdfUrlStatus`] for an answer that is
///   not a success
/// - [`PdfIngestError::PdfUrlFetch`] when the host cannot be reached or stops answering
/// - [`PdfIngestError::TooBig`] when the answer is over the limit
/// - [`PdfIngestError::PdfUrlNotAPdf`] when the answer does not start with `%PDF-`
/// - [`PdfIngestError::PdfUrlClient`] when the client that fetches cannot be made
pub(super) async fn fetch_pdf<S: Services>(
    url: &str,
    allowed: &[Url],
    max_pdf_bytes: u64,
    services: &S,
) -> Result<Vec<u8>, PdfIngestError> {
    let url = allowed_url(url, allowed)?;
    let pinned = checked_addresses(&url, services).await?;
    // A client of its own for each fetch: it is cheap next to an ingest, and the server can then
    // be made with no client that could fail.
    let mut builder = Client::builder()
        .redirect(Policy::none())
        // Without this, a proxy set in the environment would be sent requests meant for this
        // machine.
        .no_proxy()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(TOTAL_TIMEOUT);
    if let Some((host, addresses)) = &pinned {
        // The client must not look the name up again: a second answer could be an address that
        // was never checked.
        builder = builder.resolve_to_addrs(host, addresses);
    }
    let client = builder.build().map_err(PdfIngestError::PdfUrlClient)?;
    let failed = |source| PdfIngestError::PdfUrlFetch {
        url: url.to_string(),
        source,
    };
    let response = client.get(url.clone()).send().await.map_err(failed)?;
    refuse_unless_success(&response, &url)?;
    if response
        .content_length()
        .is_some_and(|length| length > max_pdf_bytes)
    {
        return Err(PdfIngestError::TooBig {
            limit: max_pdf_bytes,
        });
    }
    read_pdf(response, &url, max_pdf_bytes).await
}

/// The parsed URL, when its scheme and its origin are allowed.
fn allowed_url(text: &str, allowed: &[Url]) -> Result<Url, PdfIngestError> {
    let url = Url::parse(text).map_err(|error| PdfIngestError::NotAUrl {
        value: text.to_owned(),
        reason: error.to_string(),
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(PdfIngestError::PdfUrlScheme {
            scheme: url.scheme().to_owned(),
        });
    }
    // The origin is compared as scheme, host and port, never as text: `http://127.0.0.1@evil.test`
    // starts with an allowed address and is a different host.
    if !is_loopback(&url) && !allowed.iter().any(|origin| origin.origin() == url.origin()) {
        return Err(PdfIngestError::PdfUrlOrigin {
            origin: url.origin().ascii_serialization(),
        });
    }
    Ok(url)
}

/// The addresses to connect to when the host of `url` is a name, with that name. An IP address is
/// not checked here, because `allowed_url` lets one through only when it is on this machine or when
/// whoever runs the server listed it.
///
/// `localhost` may lead to this machine only. Any other name may lead to public addresses only,
/// because an allowed name can be pointed at an address inside a private network.
async fn checked_addresses<S: Services>(
    url: &Url,
    services: &S,
) -> Result<Option<(String, Vec<SocketAddr>)>, PdfIngestError> {
    let Some(host) = url.domain() else {
        return Ok(None);
    };
    let port = url.port_or_known_default().unwrap_or_default();
    let unresolved = |source| PdfIngestError::PdfUrlResolve {
        host: host.to_owned(),
        source,
    };
    let addresses = services
        .resolve_host(host, port)
        .await
        .map_err(unresolved)?;
    if addresses.is_empty() {
        return Err(unresolved(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "the lookup found no address",
        )));
    }
    let refused = |address: &SocketAddr| {
        if host == "localhost" {
            !address.ip().is_loopback()
        } else {
            is_internal(address.ip())
        }
    };
    if let Some(bad) = addresses.iter().find(|address| refused(address)) {
        return Err(PdfIngestError::PdfUrlInternalAddress {
            host: host.to_owned(),
            address: bad.ip(),
        });
    }
    Ok(Some((host.to_owned(), addresses)))
}

/// True for an address inside a private network or on this machine. A public host name must never
/// lead to one.
///
/// Known limitation: an IPv6 address that carries an IPv4 address in any form but the mapped one,
/// such as NAT64 (`64:ff9b::/96`) or 6to4 (`2002::/16`), counts as public.
fn is_internal(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => {
            let [first, second, ..] = ip.octets();
            // 0.0.0.0/8 means "this network", and Linux connects it to this machine. Carrier
            // networks and some private networks use the shared range 100.64.0.0/10.
            first == 0
                || ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || (first == 100 && second & 0b1100_0000 == 64)
        }
        IpAddr::V6(ip) => {
            ip.is_unspecified()
                || ip.is_loopback()
                || ip.is_unicast_link_local()
                || ip.is_unique_local()
                // `::ffff:10.0.0.1` reaches the IPv4 address it holds.
                || ip.to_ipv4_mapped().is_some_and(|mapped| is_internal(IpAddr::V4(mapped)))
        }
    }
}

fn is_loopback(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    // The host of an IPv6 address comes in brackets.
    host == "localhost"
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn refuse_unless_success(response: &Response, url: &Url) -> Result<(), PdfIngestError> {
    let status = response.status();
    if status.is_redirection() {
        return Err(PdfIngestError::PdfUrlRedirect {
            url: url.to_string(),
            status,
            location: response
                .headers()
                .get(LOCATION)
                .map(|location| String::from_utf8_lossy(location.as_bytes()).into_owned()),
        });
    }
    if !status.is_success() {
        return Err(PdfIngestError::PdfUrlStatus {
            url: url.to_string(),
            status,
        });
    }
    Ok(())
}

/// Reads the body a piece at a time and stops, dropping the connection, as soon as it is clear
/// that the body is not a PDF or would pass the limit.
async fn read_pdf(
    mut response: Response,
    url: &Url,
    max_pdf_bytes: u64,
) -> Result<Vec<u8>, PdfIngestError> {
    let failed = |source| PdfIngestError::PdfUrlFetch {
        url: url.to_string(),
        source,
    };
    let not_a_pdf = || PdfIngestError::PdfUrlNotAPdf {
        url: url.to_string(),
    };
    let mut body = Vec::new();
    while let Some(piece) = response.chunk().await.map_err(failed)? {
        if (body.len() + piece.len()) as u64 > max_pdf_bytes {
            return Err(PdfIngestError::TooBig {
                limit: max_pdf_bytes,
            });
        }
        body.extend_from_slice(&piece);
        if body.len() >= PDF_START.len() && !body.starts_with(PDF_START) {
            return Err(not_a_pdf());
        }
    }
    if body.starts_with(PDF_START) {
        Ok(body)
    } else {
        Err(not_a_pdf())
    }
}
