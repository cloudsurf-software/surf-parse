//! Optional Axum route handlers for serving surf-parse CSS.
//!
//! Enable with `features = ["axum"]` in Cargo.toml.
//!
//! # Usage
//!
//! ```ignore
//! use axum::routing::get;
//!
//! let app = axum::Router::new()
//!     .route("/static/css/surfdoc.css", get(surf_parse::serve::surfdoc_css));
//! ```

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::SURFDOC_CSS;

/// Serve the unified SurfDoc CSS with correct Content-Type and cache headers.
///
/// Returns the full `surfdoc.css` (app chrome + content rendering).
/// Consumers should mount this at their preferred CSS path.
pub async fn surfdoc_css() -> Response {
    css_response(SURFDOC_CSS)
}

fn css_response(css: &'static str) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        css,
    )
        .into_response()
}
