//! Control token authentication middleware.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

use crate::AppState;

/// Middleware that validates the Bearer token against the control_token.
pub async fn require_token(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // CORS preflight requests cannot include Authorization headers; let them
    // through so the browser receives the CORS response and follows up with
    // the real (authenticated) request.
    if req.method() == Method::OPTIONS {
        return Ok(next.run(req).await);
    }

    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header.as_bytes()[7..];
            let expected = state.control_token.as_bytes();
            // Constant-time comparison to prevent timing side-channel attacks.
            let matches = token.len() == expected.len()
                && token
                    .iter()
                    .zip(expected.iter())
                    .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                    == 0;
            // In TCP dev mode, also accept the literal `dev-token` so the
            // Vite-served web UI can authenticate without reading the
            // on-disk control token. The UDS production path never sets
            // dev_mode, so this bypass is dev-only by construction.
            let dev_match = state.dev_mode && token == b"dev-token";
            if matches || dev_match {
                Ok(next.run(req).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}
