//! Static assets for the web frontend.
//!
//! Serves the SvelteKit client build from disk, negotiating the pre-compressed
//! siblings (.br, .gz, .zst) that `web/scripts/compress-assets.ts` writes.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::response::Response;
use std::path::Path;
use std::sync::LazyLock;
use tower_http::services::ServeDir;

/// Client build directory, resolved against the working directory the image sets to /app.
const ASSETS_DIR: &str = "web/build/client";

static ASSETS: LazyLock<ServeDir> = LazyLock::new(|| serve_dir(assets_dir()));

fn serve_dir(dir: &Path) -> ServeDir {
    // Index serving is off so a directory request cannot answer ahead of the SSR server.
    ServeDir::new(dir)
        .append_index_html_on_directories(false)
        .precompressed_br()
        .precompressed_gzip()
        .precompressed_zstd()
}

pub fn assets_dir() -> &'static Path {
    Path::new(ASSETS_DIR)
}

/// Set appropriate `Cache-Control` header based on the asset path.
///
/// SvelteKit outputs fingerprinted assets under `_app/immutable/` which are
/// safe to cache indefinitely. Other assets get shorter cache durations.
fn set_cache_control(headers: &mut HeaderMap, path: &str) {
    let cache_control = if path.contains("immutable/") {
        // SvelteKit fingerprinted assets -- cache forever
        "public, max-age=31536000, immutable"
    } else if path.ends_with(".html") {
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

/// Serve a static asset, or `None` when the path is not one.
///
/// `ServeDir` supplies content negotiation, ETag, `Last-Modified`, range requests and
/// traversal rejection; `Cache-Control` it leaves alone, so the caching policy that keeps
/// these off the origin is applied here.
pub async fn try_serve_asset(method: &Method, uri: &Uri, headers: &HeaderMap) -> Option<Response> {
    serve(ASSETS.clone(), method, uri, headers).await
}

async fn serve(
    mut assets: ServeDir,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
) -> Option<Response> {
    if method != Method::GET && method != Method::HEAD {
        return None;
    }

    let mut request = Request::new(Body::empty());
    *request.method_mut() = method.clone();
    *request.uri_mut() = uri.clone();
    *request.headers_mut() = headers.clone();

    // try_call surfaces the io error; the Service impl would turn a missing file into a 404
    // and an unreadable one into a 500, and a 500 must not inherit the caching below.
    let response = match assets.try_call(request).await {
        Ok(response) => response,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            tracing::warn!(path = uri.path(), %error, "static asset read failed");
            return None;
        }
    };

    // Directories and rejected traversals come back as a 404 response rather than an error.
    if response.status() == StatusCode::NOT_FOUND {
        return None;
    }

    let (mut parts, body) = response.into_parts();
    // Ranges and failed preconditions describe one exchange, not the resource's lifetime.
    if parts.status.is_success() || parts.status == StatusCode::NOT_MODIFIED {
        set_cache_control(&mut parts.headers, uri.path());
    }
    Some(Response::from_parts(parts, Body::new(body)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert2::check;
    use std::fs;
    use std::path::PathBuf;

    /// Mirrors a client build: a fingerprinted asset with a brotli sibling, and an
    /// unfingerprinted one with no variant at all.
    fn fixture(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("banner-assets-{}-{name}", std::process::id()));
        fs::create_dir_all(dir.join("_app/immutable")).unwrap();
        fs::write(dir.join("_app/immutable/app.abc123.js"), "console.log(1)").unwrap();
        fs::write(dir.join("_app/immutable/app.abc123.js.br"), b"brotli-bytes").unwrap();
        fs::write(dir.join("_app/immutable/app.abc123.js.zst"), b"zstd-bytes").unwrap();
        fs::write(dir.join("favicon.ico"), "icon").unwrap();
        // Present only so the directory test proves index serving is off; a prerendered
        // route would otherwise let ServeDir answer "/" ahead of the SSR server.
        fs::write(dir.join("index.html"), "<html></html>").unwrap();
        dir
    }

    async fn get(dir: &Path, path: &str, headers: HeaderMap) -> Option<Response> {
        let uri: Uri = path.parse().unwrap();
        serve(serve_dir(dir), &Method::GET, &uri, &headers).await
    }

    fn accepting(encoding: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT_ENCODING, encoding.parse().unwrap());
        headers
    }

    #[tokio::test]
    async fn serves_immutable_assets_with_a_year_of_cache() {
        let dir = fixture("immutable");
        let response = get(&dir, "/_app/immutable/app.abc123.js", HeaderMap::new())
            .await
            .expect("fingerprinted asset should be served");

        check!(response.status() == StatusCode::OK);
        check!(response.headers()[header::CACHE_CONTROL] == "public, max-age=31536000, immutable");
    }

    #[tokio::test]
    async fn unfingerprinted_assets_get_a_shorter_ttl() {
        let dir = fixture("ttl");
        let response = get(&dir, "/favicon.ico", HeaderMap::new())
            .await
            .expect("favicon should be served");

        check!(response.headers()[header::CACHE_CONTROL] == "public, max-age=2592000");
    }

    /// Vary has to be set even when the response is uncompressed, or a shared cache can hand
    /// brotli to a client that never asked for it.
    #[tokio::test]
    async fn negotiates_precompressed_variants() {
        let dir = fixture("negotiate");
        let compressed = get(&dir, "/_app/immutable/app.abc123.js", accepting("br, gzip"))
            .await
            .expect("asset should be served");

        check!(compressed.headers()[header::CONTENT_ENCODING] == "br");
        check!(compressed.headers()[header::VARY] == "accept-encoding");
        check!(compressed.headers()[header::CONTENT_TYPE] == "text/javascript");

        // zstd outranks brotli at equal quality, matching what the old hand-rolled
        // negotiation preferred.
        let zstd = get(
            &dir,
            "/_app/immutable/app.abc123.js",
            accepting("gzip, br, zstd"),
        )
        .await
        .expect("asset should be served");

        check!(zstd.headers()[header::CONTENT_ENCODING] == "zstd");

        // A client that only takes gzip gets the original, since no .gz sibling exists.
        let gzip_only = get(&dir, "/_app/immutable/app.abc123.js", accepting("gzip"))
            .await
            .expect("asset should be served");

        check!(gzip_only.headers().get(header::CONTENT_ENCODING).is_none());

        let plain = get(&dir, "/_app/immutable/app.abc123.js", HeaderMap::new())
            .await
            .expect("asset should be served");

        check!(plain.headers().get(header::CONTENT_ENCODING).is_none());
        check!(plain.headers()[header::VARY] == "accept-encoding");
    }

    /// A miss falls through to the SSR proxy rather than answering 404 itself.
    #[tokio::test]
    async fn misses_fall_through() {
        let dir = fixture("miss");

        check!(
            get(&dir, "/courses/202620/12345", HeaderMap::new())
                .await
                .is_none()
        );
        check!(
            get(&dir, "/_app/immutable/gone.js", HeaderMap::new())
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn directories_do_not_answer() {
        let dir = fixture("dirs");

        check!(get(&dir, "/", HeaderMap::new()).await.is_none());
        check!(
            get(&dir, "/_app/immutable", HeaderMap::new())
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn rejects_traversal_outside_the_build() {
        let dir = fixture("traversal");

        check!(
            get(&dir, "/../../etc/passwd", HeaderMap::new())
                .await
                .is_none()
        );
        check!(
            get(&dir, "/_app/../../../etc/passwd", HeaderMap::new())
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn non_get_methods_fall_through() {
        let dir = fixture("methods");
        let uri: Uri = "/favicon.ico".parse().unwrap();

        check!(
            serve(serve_dir(&dir), &Method::POST, &uri, &HeaderMap::new())
                .await
                .is_none()
        );
    }

    /// A range describes one exchange, so it must not carry the resource's cache policy,
    /// and an unsatisfiable one must not be cached at all.
    #[tokio::test]
    async fn range_requests_carry_the_right_headers() {
        let dir = fixture("ranges");

        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, "bytes=0-1".parse().unwrap());
        let partial = get(&dir, "/favicon.ico", headers)
            .await
            .expect("range should be served");

        check!(partial.status() == StatusCode::PARTIAL_CONTENT);
        // Content-Range is also what stops CompressionLayer from recompressing the slice.
        check!(partial.headers().contains_key(header::CONTENT_RANGE));
        check!(partial.headers()[header::CACHE_CONTROL] == "public, max-age=2592000");

        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, "bytes=9999-".parse().unwrap());
        let unsatisfiable = get(&dir, "/favicon.ico", headers)
            .await
            .expect("unsatisfiable range should still respond");

        check!(unsatisfiable.status() == StatusCode::RANGE_NOT_SATISFIABLE);
        check!(unsatisfiable.headers().get(header::CACHE_CONTROL).is_none());
    }

    #[tokio::test]
    async fn revalidates_with_the_returned_etag() {
        let dir = fixture("etag");
        let first = get(&dir, "/favicon.ico", HeaderMap::new())
            .await
            .expect("favicon should be served");
        let etag = first.headers()[header::ETAG].clone();

        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, etag);
        let second = get(&dir, "/favicon.ico", headers)
            .await
            .expect("revalidation should be served");

        check!(second.status() == StatusCode::NOT_MODIFIED);
    }
}
