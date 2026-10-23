//! Pictures arrive through the authenticated server instead of local files.

pub(crate) struct EhttpFetch;

impl gui::media::Fetch for EhttpFetch {
    fn get(&self, url: &str, done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>) {
        let mut request = ehttp::Request::get(url.strip_prefix('/').unwrap_or(url));
        request.mode = ehttp::Mode::SameOrigin;
        ehttp::fetch(request, move |result| {
            done(result.and_then(|response| {
                if response.ok {
                    Ok(response.bytes)
                } else {
                    Err(format!("picture request failed: HTTP {}", response.status))
                }
            }));
        });
    }
}
