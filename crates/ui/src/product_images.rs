//! Product thumbnails are retrieved through the Worker's authenticated media
//! proxy, never by sending the session token to an image host.
use std::collections::HashMap;
use std::time::Duration;

use atlas_api::ApiClient;
use atlas_core::jobs::{Job, JobRunner};
use sha2::{Digest, Sha256};

type ImageJob = Job<(String, Option<(String, Vec<u8>)>)>;

#[derive(Default)]
pub(crate) struct ProductImages {
    // None records a failed fetch, so repainting does not retry indefinitely.
    entries: HashMap<String, Option<String>>,
    job: Option<ImageJob>,
    evict: Vec<String>,
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
}

impl ProductImages {
    pub fn clear(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
        self.job = None;
        self.evict
            .extend(self.entries.drain().filter_map(|(_, uri)| uri));
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        for uri in self.evict.drain(..) {
            ctx.forget_image(&uri);
        }
        if let Some((key, image)) = self.job.as_ref().and_then(Job::poll) {
            self.job = None;
            self.cancel = None;
            let uri = image.map(|(uri, bytes)| {
                ctx.include_bytes(uri.clone(), bytes);
                uri
            });
            self.entries.insert(key, uri);
            ctx.request_repaint();
        }
        if self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    pub fn get(&mut self, source: &str, client: &ApiClient, runner: &JobRunner) -> Option<String> {
        if let Some(value) = self.entries.get(source) {
            return value.clone();
        }
        // Only one request at a time, and only for visible rows. Bound both
        // the cache and repeated failures in very large catalogues.
        if self.job.is_some() || self.entries.len() >= 300 {
            return None;
        }
        let Some(parsed) = valid_source(source) else {
            self.entries.insert(source.to_owned(), None);
            return None;
        };
        let token = client.token()?;
        let key = source.to_owned();
        let base = client.base_url().to_owned();
        let fingerprint = client.fingerprint().to_owned();
        let mut proxy = match url::Url::parse(&format!("{base}/api/media/proxy")) {
            Ok(url) => url,
            Err(_) => return None,
        };
        proxy.query_pairs_mut().append_pair("url", parsed.as_str());
        let (cancel, cancelled) = tokio::sync::oneshot::channel();
        self.cancel = Some(cancel);
        self.job = Some(runner.spawn(async move {
            let image = tokio::select! {
                result = fetch(proxy, token, fingerprint, &key) => result,
                _ = cancelled => None,
            };
            (key, image)
        }));
        None
    }
}

async fn fetch(
    proxy: url::Url,
    token: String,
    fingerprint: String,
    key: &str,
) -> Option<(String, Vec<u8>)> {
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
        .ok()?;
    let mut response = http
        .get(proxy)
        .bearer_auth(token)
        .header("X-Atlas-Client", "native")
        .header("X-Device-Fingerprint", fingerprint)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)?
        .to_str()
        .ok()?
        .split(';')
        .next()?
        .trim();
    match mime {
        "image/png" | "image/jpeg" | "image/webp" | "image/gif" => {}
        _ => return None,
    };
    const LIMIT: usize = 4 * 1024 * 1024;
    if response
        .content_length()
        .is_some_and(|size| size > LIMIT as u64)
    {
        return None;
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if bytes.len().saturating_add(chunk.len()) > LIMIT {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    // Reject image headers declaring excessive dimensions before the UI loader
    // decompresses them. The server normally returns normalized thumbnails.
    let reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .ok()?;
    let (width, height) = reader.into_dimensions().ok()?;
    if width == 0 || height == 0 || width > 2048 || height > 2048 {
        return None;
    }
    // Cache only small thumbnails, not full-size decoded textures for every row.
    let image = image::load_from_memory(&bytes).ok()?.thumbnail(64, 64);
    let mut output = std::io::Cursor::new(Vec::new());
    image.write_to(&mut output, image::ImageFormat::Png).ok()?;
    let digest = Sha256::digest(key.as_bytes());
    Some((
        format!("bytes://product-{}.png", hex::encode(digest)),
        output.into_inner(),
    ))
}

fn valid_source(source: &str) -> Option<url::Url> {
    let url = url::Url::parse(source).ok()?;
    (url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none())
    .then_some(url)
}

#[cfg(test)]
mod tests {
    use super::valid_source;

    #[test]
    fn source_requires_https_without_credentials_or_fragment() {
        assert!(valid_source("https://cdn.example/products/a.webp").is_some());
        for url in [
            "http://cdn.example/image",
            "https://user:secret@cdn.example/image",
            "https://cdn.example/image#fragment",
            "data:image/png;base64,abc",
            "/relative",
        ] {
            assert!(valid_source(url).is_none(), "{url}");
        }
    }
}
