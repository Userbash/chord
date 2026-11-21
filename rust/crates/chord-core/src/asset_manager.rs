//! Dynamic background downloader for localization strings, regional font subsets,
//! emoji packs, and audio cues with local caching and SHA-256 validation.

use std::fmt;
use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetKind {
    LocalizationStrings,
    FontSubset,
    EmojiPack,
    AudioCue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetDescriptor {
    pub id: String,
    pub kind: AssetKind,
    pub url: String,
    pub expected_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetError {
    InvalidHash { expected: String, actual: String },
    DownloadFailed(String),
    IoError(String),
    InvalidDescriptor(String),
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHash { expected, actual } => {
                write!(f, "SHA-256 mismatch: expected {expected}, got {actual}")
            }
            Self::DownloadFailed(msg) => write!(f, "download failed: {msg}"),
            Self::IoError(msg) => write!(f, "I/O error: {msg}"),
            Self::InvalidDescriptor(msg) => write!(f, "invalid asset descriptor: {msg}"),
        }
    }
}

impl std::error::Error for AssetError {}

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait AssetTransport: Send + Sync {
    fn fetch<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, AssetError>>;
}

pub struct ReqwestAssetTransport {
    client: reqwest::Client,
}

impl ReqwestAssetTransport {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for ReqwestAssetTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetTransport for ReqwestAssetTransport {
    fn fetch<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, AssetError>> {
        Box::pin(async move {
            let resp = self
                .client
                .get(url)
                .send()
                .await
                .map_err(|e| AssetError::DownloadFailed(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(AssetError::DownloadFailed(format!(
                    "HTTP status {}",
                    resp.status()
                )));
            }
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| AssetError::DownloadFailed(e.to_string()))?;
            Ok(bytes.to_vec())
        })
    }
}

/// Computes lowercase hex-encoded SHA-256 digest of `data` without unsafe code.
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = String::with_capacity(64);
    for word in h {
        use std::fmt::Write as _;
        let _ = write!(out, "{word:08x}");
    }
    out
}

#[derive(Clone)]
pub struct AssetManager {
    cache_dir: PathBuf,
    transport: Arc<dyn AssetTransport>,
}

impl AssetManager {
    pub fn new(cache_dir: PathBuf, transport: Arc<dyn AssetTransport>) -> Self {
        Self {
            cache_dir,
            transport,
        }
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn cache_path_for(&self, id: &str) -> Result<PathBuf, AssetError> {
        if id.is_empty() {
            return Err(AssetError::InvalidDescriptor("empty asset id".to_string()));
        }
        let rel = Path::new(id);
        let mut clean = PathBuf::new();
        for comp in rel.components() {
            match comp {
                Component::Normal(part) => clean.push(part),
                _ => {
                    return Err(AssetError::InvalidDescriptor(format!(
                        "invalid path component in asset id: {id}"
                    )));
                }
            }
        }
        if clean.as_os_str().is_empty() {
            return Err(AssetError::InvalidDescriptor("empty asset id".to_string()));
        }
        Ok(self.cache_dir.join(clean))
    }

    pub async fn verify_cached(&self, desc: &AssetDescriptor) -> Result<bool, AssetError> {
        let path = self.cache_path_for(&desc.id)?;
        let expected = desc.expected_sha256.to_ascii_lowercase();
        tokio::task::spawn_blocking(move || match std::fs::read(&path) {
            Ok(bytes) => Ok(sha256_hex(&bytes) == expected),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(AssetError::IoError(e.to_string())),
        })
        .await
        .map_err(|e| AssetError::IoError(e.to_string()))?
    }

    pub async fn get_or_download(&self, desc: &AssetDescriptor) -> Result<PathBuf, AssetError> {
        let path = self.cache_path_for(&desc.id)?;
        if self.verify_cached(desc).await? {
            return Ok(path);
        }

        let bytes = self.transport.fetch(&desc.url).await?;
        let actual = sha256_hex(&bytes);
        let expected = desc.expected_sha256.to_ascii_lowercase();
        if actual != expected {
            return Err(AssetError::InvalidHash { expected, actual });
        }

        let target = path.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| AssetError::IoError(e.to_string()))?;
            }
            let tmp = target.with_extension("tmp");
            std::fs::write(&tmp, &bytes).map_err(|e| AssetError::IoError(e.to_string()))?;
            std::fs::rename(&tmp, &target).map_err(|e| AssetError::IoError(e.to_string()))?;
            Ok::<_, AssetError>(())
        })
        .await
        .map_err(|e| AssetError::IoError(e.to_string()))??;

        Ok(path)
    }

    pub fn spawn_prefetch(
        &self,
        descriptors: Vec<AssetDescriptor>,
    ) -> tokio::task::JoinHandle<Vec<Result<PathBuf, AssetError>>> {
        let mgr = self.clone();
        tokio::spawn(async move {
            let mut results = Vec::with_capacity(descriptors.len());
            for desc in descriptors {
                results.push(mgr.get_or_download(&desc).await);
            }
            results
        })
    }

    pub async fn evict(&self, id: &str) -> Result<bool, AssetError> {
        let path = self.cache_path_for(id)?;
        tokio::task::spawn_blocking(move || match std::fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(AssetError::IoError(e.to_string())),
        })
        .await
        .map_err(|e| AssetError::IoError(e.to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct MockTransport {
        responses: HashMap<String, Result<Vec<u8>, String>>,
        calls: AtomicUsize,
    }

    impl MockTransport {
        fn new(responses: HashMap<String, Result<Vec<u8>, String>>) -> Self {
            Self {
                responses,
                calls: AtomicUsize::new(0),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl AssetTransport for MockTransport {
        fn fetch<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Vec<u8>, AssetError>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                match self.responses.get(url) {
                    Some(Ok(bytes)) => Ok(bytes.clone()),
                    Some(Err(msg)) => Err(AssetError::DownloadFailed(msg.clone())),
                    None => Err(AssetError::DownloadFailed(format!("404 {url}"))),
                }
            })
        }
    }

    fn temp_cache_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("chord_asset_mgr_{label}_{nanos}"))
    }

    #[test]
    fn sha256_hex_matches_nist_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[tokio::test]
    async fn downloads_on_first_use_and_caches_with_verified_hash() {
        let dir = temp_cache_dir("first_use");
        let payload = r#"{"greeting":"こんにちは"}"#.as_bytes().to_vec();
        let hash = sha256_hex(&payload);
        let url = "https://cdn.example.test/locales/ja.json".to_string();

        let mut map = HashMap::new();
        map.insert(url.clone(), Ok(payload.clone()));
        let transport = Arc::new(MockTransport::new(map));
        let mgr = AssetManager::new(dir.clone(), transport.clone());

        let desc = AssetDescriptor {
            id: "locales/ja.json".to_string(),
            kind: AssetKind::LocalizationStrings,
            url,
            expected_sha256: hash,
        };

        let path1 = mgr.get_or_download(&desc).await.expect("first download");
        assert_eq!(transport.call_count(), 1);
        assert_eq!(std::fs::read(&path1).unwrap(), payload);

        // Second call must hit local cache without network fetch.
        let path2 = mgr.get_or_download(&desc).await.expect("cache hit");
        assert_eq!(path1, path2);
        assert_eq!(transport.call_count(), 1);
        assert!(mgr.verify_cached(&desc).await.unwrap());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn rejects_corrupted_download_and_does_not_cache() {
        let dir = temp_cache_dir("bad_hash");
        let url = "https://cdn.example.test/fonts/noto-cjk.woff2".to_string();
        let mut map = HashMap::new();
        map.insert(url.clone(), Ok(b"corrupted-font-bytes".to_vec()));
        let transport = Arc::new(MockTransport::new(map));
        let mgr = AssetManager::new(dir.clone(), transport);

        let desc = AssetDescriptor {
            id: "fonts/noto-cjk.woff2".to_string(),
            kind: AssetKind::FontSubset,
            url,
            expected_sha256: "0".repeat(64),
        };

        let err = mgr.get_or_download(&desc).await.unwrap_err();
        assert!(matches!(err, AssetError::InvalidHash { .. }));
        assert!(!mgr.verify_cached(&desc).await.unwrap());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn redownloads_when_cached_file_is_tampered() {
        let dir = temp_cache_dir("tampered");
        let payload = b"valid-audio-cue".to_vec();
        let hash = sha256_hex(&payload);
        let url = "https://cdn.example.test/audio/ringtone.ogg".to_string();

        let mut map = HashMap::new();
        map.insert(url.clone(), Ok(payload.clone()));
        let transport = Arc::new(MockTransport::new(map));
        let mgr = AssetManager::new(dir.clone(), transport.clone());

        let desc = AssetDescriptor {
            id: "audio/ringtone.ogg".to_string(),
            kind: AssetKind::AudioCue,
            url,
            expected_sha256: hash,
        };

        let path = mgr.get_or_download(&desc).await.expect("initial");
        assert_eq!(transport.call_count(), 1);

        // Tamper with cached file.
        std::fs::write(&path, b"tampered").unwrap();
        assert!(!mgr.verify_cached(&desc).await.unwrap());

        // Next get_or_download must re-fetch and restore valid bytes.
        let path2 = mgr.get_or_download(&desc).await.expect("re-download");
        assert_eq!(transport.call_count(), 2);
        assert_eq!(std::fs::read(&path2).unwrap(), payload);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn background_prefetch_downloads_multiple_asset_kinds() {
        let dir = temp_cache_dir("prefetch");
        let font_bytes = b"font-subset-data".to_vec();
        let emoji_bytes = b"emoji-pack-data".to_vec();
        let font_url = "https://cdn.example.test/fonts/arabic.woff2".to_string();
        let emoji_url = "https://cdn.example.test/emoji/pack1.bin".to_string();

        let mut map = HashMap::new();
        map.insert(font_url.clone(), Ok(font_bytes.clone()));
        map.insert(emoji_url.clone(), Ok(emoji_bytes.clone()));
        let transport = Arc::new(MockTransport::new(map));
        let mgr = AssetManager::new(dir.clone(), transport.clone());

        let handle = mgr.spawn_prefetch(vec![
            AssetDescriptor {
                id: "fonts/arabic.woff2".to_string(),
                kind: AssetKind::FontSubset,
                url: font_url,
                expected_sha256: sha256_hex(&font_bytes),
            },
            AssetDescriptor {
                id: "emoji/pack1.bin".to_string(),
                kind: AssetKind::EmojiPack,
                url: emoji_url,
                expected_sha256: sha256_hex(&emoji_bytes),
            },
        ]);

        let results = handle.await.expect("join");
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_ok()));
        assert_eq!(transport.call_count(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn rejects_path_traversal_and_supports_eviction() {
        let dir = temp_cache_dir("traversal");
        let transport = Arc::new(MockTransport::new(HashMap::new()));
        let mgr = AssetManager::new(dir.clone(), transport);

        assert!(matches!(
            mgr.cache_path_for("../etc/passwd"),
            Err(AssetError::InvalidDescriptor(_))
        ));
        assert!(matches!(
            mgr.cache_path_for("/abs/path"),
            Err(AssetError::InvalidDescriptor(_))
        ));
        assert!(matches!(
            mgr.cache_path_for(""),
            Err(AssetError::InvalidDescriptor(_))
        ));

        let valid_path = mgr.cache_path_for("locales/en-US.json").unwrap();
        std::fs::create_dir_all(valid_path.parent().unwrap()).unwrap();
        std::fs::write(&valid_path, b"{}").unwrap();
        assert!(mgr.evict("locales/en-US.json").await.unwrap());
        assert!(!mgr.evict("locales/en-US.json").await.unwrap());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
