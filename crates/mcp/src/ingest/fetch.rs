//! Fetches a PDF that an agent named by URL: only from an allowed origin, with no redirect, and no
//! more bytes than the size limit.

use std::net::IpAddr;
use std::time::Duration;

use reqwest::redirect::Policy;
use reqwest::{Client, Response, Url, header::LOCATION};

use super::{PDF_START, PdfIngestError};

/// How long the server waits to connect, so that a host which never answers cannot hold the
/// ingest slot.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the whole fetch may take, from the first byte sent to the last byte read.
const TOTAL_TIMEOUT: Duration = Duration::from_secs(120);

/// The bytes at `url`, when the URL is on this machine or on one of the `allowed` origins, the
/// host answers with a PDF, and the PDF is no bigger than `max_pdf_bytes`. Nothing is connected to
/// before the origin is checked, and a redirect is refused and not followed.
///
/// # Errors
/// - [`PdfIngestError::NotAUrl`], [`PdfIngestError::PdfUrlScheme`] and
///   [`PdfIngestError::PdfUrlOrigin`] for a URL that is refused before any connection
/// - [`PdfIngestError::PdfUrlRedirect`] and [`PdfIngestError::PdfUrlStatus`] for an answer that is
///   not a success
/// - [`PdfIngestError::PdfUrlFetch`] when the host cannot be reached or stops answering
/// - [`PdfIngestError::TooBig`] when the answer is over the limit
/// - [`PdfIngestError::PdfUrlNotAPdf`] when the answer does not start with `%PDF-`
/// - [`PdfIngestError::PdfUrlClient`] when the client that fetches cannot be made
pub(super) async fn fetch_pdf(
    url: &str,
    allowed: &[Url],
    max_pdf_bytes: u64,
) -> Result<Vec<u8>, PdfIngestError> {
    let url = allowed_url(url, allowed)?;
    // A client of its own for each fetch: it is cheap next to an ingest, and the server can then
    // be made with no client that could fail.
    let client = Client::builder()
        .redirect(Policy::none())
        // Without this, a proxy set in the environment would be sent requests meant for this
        // machine.
        .no_proxy()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(TOTAL_TIMEOUT)
        .build()
        .map_err(PdfIngestError::PdfUrlClient)?;
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
