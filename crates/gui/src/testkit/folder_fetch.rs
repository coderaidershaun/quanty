//! Sample pictures are fetched through their server URLs without a running web server.

use std::path::{Component, Path, PathBuf};

use crate::media::Fetch;

pub struct FolderFetch {
    root: PathBuf,
}

impl FolderFetch {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn read(&self, url: &str) -> Result<Vec<u8>, String> {
        let relative = url
            .strip_prefix("/content/")
            .ok_or_else(|| format!("not a content URL: {url}"))?;
        let relative = relative.split('?').next().unwrap_or(relative);
        let relative = percent_encoding::percent_decode_str(relative)
            .decode_utf8()
            .map_err(|error| error.to_string())?;
        let path = Path::new(relative.as_ref());
        if !path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        {
            return Err(format!("not a relative content path: {url}"));
        }
        let root = self
            .root
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let path = root
            .join(path)
            .canonicalize()
            .map_err(|error| error.to_string())?;
        if !path.starts_with(&root) {
            return Err(format!("picture leaves the content folder: {url}"));
        }
        std::fs::read(path).map_err(|error| error.to_string())
    }
}

impl Fetch for FolderFetch {
    fn get(&self, url: &str, done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>) {
        done(self.read(url));
    }
}
