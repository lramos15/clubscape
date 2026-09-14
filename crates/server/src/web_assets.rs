use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Component, Path},
};

use axum::{
    body::{Body, Bytes},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::Response,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub(crate) enum WebAssetError {
    #[error("web bundle filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("web bundle manifest is invalid")]
    Manifest(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(&'static str),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    files: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    url: String,
    path: String,
    sha256: String,
    content_type: String,
}

struct Asset {
    bytes: Bytes,
    content_type: HeaderValue,
    etag: HeaderValue,
    length: HeaderValue,
}

pub(crate) struct WebAssets {
    files: BTreeMap<String, Asset>,
}

fn public_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.contains(['\\', '%', '?', '#'])
        && !value.chars().any(char::is_control)
        && Path::new(value).components().all(|part| {
            matches!(part, Component::Normal(name) if !name.to_string_lossy().starts_with('.'))
        })
}

fn allowed_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some(
            "html"
                | "js"
                | "css"
                | "wasm"
                | "json"
                | "png"
                | "jpg"
                | "jpeg"
                | "webp"
                | "ogg"
                | "flac"
                | "wav"
                | "glb"
                | "bin"
                | "ktx2"
                | "woff"
                | "woff2"
                | "ttf"
                | "svg"
        )
    )
}

impl WebAssets {
    pub(crate) fn load(root: &Path) -> Result<Self, WebAssetError> {
        let root = root.canonicalize()?;
        let manifest_path = root.join("clubscape-web.json");
        if fs::symlink_metadata(&manifest_path)?
            .file_type()
            .is_symlink()
            || fs::metadata(&manifest_path)?.len() > 2 * 1024 * 1024
        {
            return Err(WebAssetError::Invalid(
                "web manifest must be a bounded regular file",
            ));
        }
        let mut manifest_bytes = Vec::new();
        fs::File::open(manifest_path)?
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut manifest_bytes)?;
        if manifest_bytes.len() > 2 * 1024 * 1024 {
            return Err(WebAssetError::Invalid(
                "web manifest exceeds its byte budget",
            ));
        }
        let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
        if manifest.schema_version != 1
            || manifest.files.is_empty()
            || manifest.files.len() > 20_000
        {
            return Err(WebAssetError::Invalid(
                "web manifest version or file count is invalid",
            ));
        }
        let mut files = BTreeMap::new();
        let mut total = 0_u64;
        for entry in manifest.files {
            let route = entry.url.strip_prefix('/').ok_or(WebAssetError::Invalid(
                "asset URLs must be same-origin paths",
            ))?;
            if (!route.is_empty() && !public_path(route))
                || entry.url.starts_with("/v1/")
                || entry.url == "/healthz"
                || !public_path(&entry.path)
                || !allowed_extension(Path::new(&entry.path))
                || entry.sha256.len() != 64
                || !entry
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(WebAssetError::Invalid(
                    "asset path, reserved route or hash is invalid",
                ));
            }
            let mut path = root.clone();
            for part in Path::new(&entry.path).components() {
                path.push(part);
                if fs::symlink_metadata(&path)?.file_type().is_symlink() {
                    return Err(WebAssetError::Invalid(
                        "web assets cannot traverse symbolic links",
                    ));
                }
            }
            let metadata = fs::metadata(&path)?;
            if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
                return Err(WebAssetError::Invalid(
                    "web asset is not a bounded regular file",
                ));
            }
            total = total
                .checked_add(metadata.len())
                .ok_or(WebAssetError::Invalid("web bundle size overflow"))?;
            if total > MAX_TOTAL_BYTES {
                return Err(WebAssetError::Invalid(
                    "web bundle exceeds its memory budget",
                ));
            }
            let mut bytes = Vec::new();
            fs::File::open(path)?
                .take(MAX_FILE_BYTES + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 != metadata.len()
                || format!("{:x}", Sha256::digest(&bytes)) != entry.sha256
            {
                return Err(WebAssetError::Invalid(
                    "web asset size/hash does not match the manifest",
                ));
            }
            let content_type = HeaderValue::from_str(&entry.content_type)
                .map_err(|_| WebAssetError::Invalid("web content type is invalid"))?;
            let etag = HeaderValue::from_str(&format!("\"{}\"", entry.sha256))
                .map_err(|_| WebAssetError::Invalid("web ETag is invalid"))?;
            let length = HeaderValue::from_str(&bytes.len().to_string())
                .map_err(|_| WebAssetError::Invalid("web asset length is invalid"))?;
            if files
                .insert(
                    entry.url,
                    Asset {
                        bytes: Bytes::from(bytes),
                        content_type,
                        etag,
                        length,
                    },
                )
                .is_some()
            {
                return Err(WebAssetError::Invalid("duplicate public asset route"));
            }
        }
        if files
            .get("/")
            .is_none_or(|asset| !asset.content_type.as_bytes().starts_with(b"text/html"))
        {
            return Err(WebAssetError::Invalid(
                "web bundle requires an explicit HTML entry at /",
            ));
        }
        Ok(Self { files })
    }

    pub(crate) fn response(
        &self,
        path: &str,
        method: &Method,
        headers: &HeaderMap,
    ) -> Option<Response> {
        let asset = self.files.get(path)?;
        let mut response = if method != Method::GET && method != Method::HEAD {
            let mut response = Response::new(Body::empty());
            *response.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
            response
                .headers_mut()
                .insert(header::ALLOW, HeaderValue::from_static("GET, HEAD"));
            response
        } else if headers.get(header::IF_NONE_MATCH) == Some(&asset.etag) {
            let mut response = Response::new(Body::empty());
            *response.status_mut() = StatusCode::NOT_MODIFIED;
            response
        } else {
            let body = if method == Method::HEAD {
                Body::empty()
            } else {
                Body::from(asset.bytes.clone())
            };
            let mut response = Response::new(body);
            response
                .headers_mut()
                .insert(header::CONTENT_LENGTH, asset.length.clone());
            response
        };
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, asset.content_type.clone());
        response
            .headers_mut()
            .insert(header::ETAG, asset.etag.clone());
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        response.headers_mut().insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        response.headers_mut().insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'self' data:; media-src 'self' blob:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'"),
        );
        response.headers_mut().insert(
            "cross-origin-opener-policy",
            HeaderValue::from_static("same-origin"),
        );
        response.headers_mut().insert(
            "cross-origin-embedder-policy",
            HeaderValue::from_static("require-corp"),
        );
        Some(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/web-asset-tests")
                .join(uuid::Uuid::new_v4().to_string());
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join("index.html"), b"<p>web bundle fixture</p>").unwrap();
            Self(path)
        }
        fn manifest(&self, path: &str, url: &str, hash: Option<&str>) {
            let hash = hash
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{:x}", Sha256::digest(b"<p>web bundle fixture</p>")));
            fs::write(self.0.join("clubscape-web.json"), serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "files": [{"url":url,"path":path,"sha256":hash,"content_type":"text/html; charset=utf-8"}]
            })).unwrap()).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[tokio::test]
    async fn hashes_are_pinned_at_startup_and_only_manifest_paths_are_served() {
        let fixture = Fixture::new();
        fixture.manifest("index.html", "/", None);
        let assets = WebAssets::load(&fixture.0).unwrap();
        fs::write(fixture.0.join("index.html"), "changed after startup").unwrap();
        let response = assets
            .response("/", &Method::GET, &HeaderMap::new())
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::X_CONTENT_TYPE_OPTIONS],
            "nosniff"
        );
        let etag = response.headers()[header::ETAG].clone();
        assert_eq!(
            axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap(),
            b"<p>web bundle fixture</p>"[..]
        );
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, etag);
        assert_eq!(
            assets
                .response("/", &Method::GET, &headers)
                .unwrap()
                .status(),
            StatusCode::NOT_MODIFIED
        );
        assert!(assets.response("/.env", &Method::GET, &headers).is_none());
        assert!(
            assets
                .response("/%2e%2e/secrets", &Method::GET, &headers)
                .is_none()
        );
        assert_eq!(
            assets
                .response("/", &Method::POST, &headers)
                .unwrap()
                .status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
    }

    #[test]
    fn invalid_paths_hashes_and_reserved_routes_fail_startup() {
        let fixture = Fixture::new();
        for (path, url) in [
            ("../index.html", "/"),
            (".env", "/"),
            ("index.html", "/v1/rpc"),
            ("index.html", "/healthz"),
            ("index.html", "/../file"),
        ] {
            fixture.manifest(path, url, None);
            assert!(WebAssets::load(&fixture.0).is_err(), "{path} {url}");
        }
        fixture.manifest("index.html", "/", Some(&"a".repeat(64)));
        assert!(WebAssets::load(&fixture.0).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_cannot_publish_private_files() {
        let fixture = Fixture::new();
        std::os::unix::fs::symlink("index.html", fixture.0.join("linked.html")).unwrap();
        fixture.manifest("linked.html", "/", None);
        assert!(WebAssets::load(&fixture.0).is_err());
    }

    #[tokio::test]
    #[ignore = "requires isolated PostgreSQL; run just test-integration"]
    async fn configured_bundle_is_served_over_the_real_account_listener() {
        use sqlx::postgres::PgConnectOptions;
        use std::{net::IpAddr, str::FromStr, time::Duration};

        let url = std::env::var("CLUBSCAPE_TEST_DATABASE_URL")
            .expect("isolated test database is required");
        let database = PgConnectOptions::from_str(&url).expect("valid test database URL");
        assert_eq!(database.get_database(), Some("clubscape_m1_test"));
        assert!(database.get_host().parse::<IpAddr>().unwrap().is_loopback());
        let fixture = Fixture::new();
        fixture.manifest("index.html", "/", None);
        let config = crate::Config::new(&url, "127.0.0.1:0", Some("web-bundle-test"))
            .unwrap()
            .with_web_root(&fixture.0)
            .unwrap();
        let service = crate::Service::bind(config).await.unwrap();
        let origin = format!("http://{}", service.local_addr());
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let running = tokio::spawn(service.serve(async {
            let _ = stopped.await;
        }));
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let observations = async {
            let page = client.get(&origin).send().await?;
            let status = page.status();
            let etag = page.headers()[header::ETAG].clone();
            let body = page.text().await?;
            let unchanged = client
                .get(&origin)
                .header(header::IF_NONE_MATCH, etag)
                .send()
                .await?
                .status();
            let private = client.get(format!("{origin}/.env")).send().await?.status();
            let method = client.post(&origin).send().await?.status();
            Ok::<_, reqwest::Error>((status, body, unchanged, private, method))
        }
        .await;
        stop.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(10), running)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let (status, body, unchanged, private, method) = observations.unwrap();
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "<p>web bundle fixture</p>");
        assert_eq!(unchanged, StatusCode::NOT_MODIFIED);
        assert_eq!(private, StatusCode::NOT_FOUND);
        assert_eq!(method, StatusCode::METHOD_NOT_ALLOWED);
    }
}
