//! Embedded assets for the web frontend.
//!
//! Serves static assets embedded into the binary at compile time using rust-embed.
//! Supports content negotiation for pre-compressed variants (.br, .gz, .zst)
//! generated at build time by `web/scripts/compress-assets.ts`.

use axum::http::{HeaderMap, HeaderValue, header};
use dashmap::DashMap;
use rapidhash::v3::rapidhash_v3;
use rust_embed::RustEmbed;
use std::fmt;
use std::sync::LazyLock;

use super::encoding::{COMPRESSION_MIN_SIZE, ContentEncoding, parse_accepted_encodings};

/// Embedded web assets from the SSR build's client directory
#[derive(RustEmbed)]
#[folder = "web/build/client/"]
#[include = "*"]
#[exclude = "*.map"]
pub struct WebAssets;

/// RapidHash hash type for asset content (u64 native output size)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetHash(u64);

impl AssetHash {
    pub fn new(hash: u64) -> Self {
        Self(hash)
    }

    pub fn to_hex(&self) -> String {
        format!("{:016x}", self.0)
    }

    /// Get the hash as a quoted hex string (for ETag headers)
    pub fn quoted(&self) -> String {
        format!("\"{}\"", self.to_hex())
    }
}

impl fmt::Display for AssetHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Metadata for an asset including MIME type and RapidHash hash
#[derive(Debug, Clone)]
pub struct AssetMetadata {
    pub mime_type: Option<String>,
    pub hash: AssetHash,
}

impl AssetMetadata {
    pub fn etag_matches(&self, etag: &str) -> bool {
        let etag = etag.trim_matches('"');
        etag.len() == 16
            && u64::from_str_radix(etag, 16)
                .map(|parsed| parsed == self.hash.0)
                .unwrap_or(false)
    }
}

/// Global cache for asset metadata to avoid repeated calculations
static ASSET_CACHE: LazyLock<DashMap<String, AssetMetadata>> = LazyLock::new(DashMap::new);

/// Get cached asset metadata for a file path, caching on-demand
pub fn get_asset_metadata_cached(path: &str, content: &[u8]) -> AssetMetadata {
    if let Some(cached) = ASSET_CACHE.get(path) {
        return cached.value().clone();
    }

    let mime_type = mime_guess::from_path(path)
        .first()
        .map(|mime| mime.to_string());
    let hash = AssetHash::new(rapidhash_v3(content));
    let metadata = AssetMetadata { mime_type, hash };

    if ASSET_CACHE.len() < 1000 {
        ASSET_CACHE.insert(path.to_string(), metadata.clone());
    }

    metadata
}

/// Set appropriate `Cache-Control` header based on the asset path.
///
/// SvelteKit outputs fingerprinted assets under `_app/immutable/` which are
/// safe to cache indefinitely. Other assets get shorter cache durations.
fn set_cache_control(headers: &mut HeaderMap, path: &str) {
    let cache_control = if path.contains("immutable/") {
        // SvelteKit fingerprinted assets — cache forever
        "public, max-age=31536000, immutable"
    } else if path == "index.html" || path.ends_with(".html") {
        "public, max-age=300"
    } else {
        match path.rsplit_once('.').map(|(_, ext)| ext) {
            Some("css" | "js") => "public, max-age=86400",
            Some("png" | "jpg" | "jpeg" | "gif" | "svg" | "ico") => "public, max-age=2592000",
            _ => "public, max-age=3600",
        }
    };

    if let Ok(value) = HeaderValue::from_str(cache_control) {
        headers.insert(header::CACHE_CONTROL, value);
    }
}

/// Serve an embedded asset with content encoding negotiation.
///
/// Tries pre-compressed variants (.br, .gz, .zst) in the order preferred by
/// the client's `Accept-Encoding` header, falling back to the uncompressed
/// original. Returns `None` if the asset doesn't exist at all.
pub fn try_serve_asset_with_encoding(
    path: &str,
    request_headers: &HeaderMap,
) -> Option<axum::response::Response> {
    use axum::response::IntoResponse;

    let asset_path = path.strip_prefix('/').unwrap_or(path);

    // Get the uncompressed original first (for metadata: MIME type, ETag)
    let original = WebAssets::get(asset_path)?;
    let metadata = get_asset_metadata_cached(asset_path, &original.data);

    // Check ETag for conditional requests (304 Not Modified)
    if let Some(etag) = request_headers.get(header::IF_NONE_MATCH)
        && etag.to_str().is_ok_and(|s| metadata.etag_matches(s))
    {
        return Some(axum::http::StatusCode::NOT_MODIFIED.into_response());
    }

    let mime_type = metadata
        .mime_type
        .unwrap_or_else(|| "application/octet-stream".to_string());

    // Only attempt pre-compressed variants for files above the compression
    // threshold — the build script skips smaller files too.
    let accepted_encodings = if original.data.len() >= COMPRESSION_MIN_SIZE {
        parse_accepted_encodings(request_headers)
    } else {
        vec![ContentEncoding::Identity]
    };

    for encoding in &accepted_encodings {
        if *encoding == ContentEncoding::Identity {
            continue;
        }

        let compressed_path = format!("{}{}", asset_path, encoding.extension());
        if let Some(compressed) = WebAssets::get(&compressed_path) {
            let mut response_headers = HeaderMap::new();

            if let Ok(ct) = HeaderValue::from_str(&mime_type) {
                response_headers.insert(header::CONTENT_TYPE, ct);
            }
            if let Some(ce) = encoding.header_value() {
                response_headers.insert(header::CONTENT_ENCODING, ce);
            }
            if let Ok(etag_val) = HeaderValue::from_str(&metadata.hash.quoted()) {
                response_headers.insert(header::ETAG, etag_val);
            }
            // Vary so caches distinguish by encoding
            response_headers.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
            set_cache_control(&mut response_headers, asset_path);

            return Some(
                (
                    axum::http::StatusCode::OK,
                    response_headers,
                    compressed.data,
                )
                    .into_response(),
            );
        }
    }

    // No compressed variant found — serve uncompressed original
    let mut response_headers = HeaderMap::new();
    if let Ok(ct) = HeaderValue::from_str(&mime_type) {
        response_headers.insert(header::CONTENT_TYPE, ct);
    }
    if let Ok(etag_val) = HeaderValue::from_str(&metadata.hash.quoted()) {
        response_headers.insert(header::ETAG, etag_val);
    }
    set_cache_control(&mut response_headers, asset_path);

    Some((axum::http::StatusCode::OK, response_headers, original.data).into_response())
}
