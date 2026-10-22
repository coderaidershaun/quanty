//! Picture bytes come from the host, so the cache works in the browser and in a test window.

pub trait Fetch {
    fn get(&self, url: &str, done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>);
}

pub struct UnavailableFetch;

impl Fetch for UnavailableFetch {
    fn get(&self, _url: &str, done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>) {
        done(Err("image fetcher is not installed".into()));
    }
}
