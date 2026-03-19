//! Local control-plane client for ClawX.
//!
//! Provides a typed Rust client that communicates with `clawx-api`
//! over a Unix Domain Socket (or localhost HTTP in dev mode).

use clawx_types::error::{ClawxError, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;
use tracing::debug;

/// Client for communicating with the ClawX service API.
#[derive(Debug, Clone)]
pub struct ControlPlaneClient {
    base_url: String,
    token: String,
    http: Client,
}

impl ControlPlaneClient {
    /// Create a client that connects via TCP (dev mode).
    pub fn new_tcp(port: u16, token: String) -> Self {
        Self {
            base_url: format!("http://127.0.0.1:{}", port),
            token,
            http: Client::new(),
        }
    }

    /// Read the control token from `~/.clawx/run/control_token`.
    pub async fn read_token(data_dir: &str) -> Result<String> {
        let path = format!("{}/run/control_token", data_dir);
        tokio::fs::read_to_string(&path)
            .await
            .map(|s| s.trim().to_string())
            .map_err(|e| ClawxError::Internal(format!("failed to read control token: {}", e)))
    }

    /// Perform a GET request and deserialize the JSON response.
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        debug!(path, "GET request");
        let resp = self
            .http
            .get(format!("{}{}", self.base_url, path))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| ClawxError::Internal(format!("HTTP request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(ClawxError::Internal(format!(
                "API returned status {}",
                resp.status()
            )));
        }

        resp.json()
            .await
            .map_err(|e| ClawxError::Internal(format!("failed to parse response: {}", e)))
    }

    /// Perform a POST request with a JSON body.
    pub async fn post<B: serde::Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        debug!(path, "POST request");
        let resp = self
            .http
            .post(format!("{}{}", self.base_url, path))
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await
            .map_err(|e| ClawxError::Internal(format!("HTTP request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(ClawxError::Internal(format!(
                "API returned status {}",
                resp.status()
            )));
        }

        resp.json()
            .await
            .map_err(|e| ClawxError::Internal(format!("failed to parse response: {}", e)))
    }

    /// Perform a PUT request with a JSON body.
    pub async fn put<B: serde::Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        debug!(path, "PUT request");
        let resp = self
            .http
            .put(format!("{}{}", self.base_url, path))
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await
            .map_err(|e| ClawxError::Internal(format!("HTTP request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(ClawxError::Internal(format!(
                "API returned status {}",
                resp.status()
            )));
        }

        resp.json()
            .await
            .map_err(|e| ClawxError::Internal(format!("failed to parse response: {}", e)))
    }

    /// Perform a DELETE request.
    pub async fn delete(&self, path: &str) -> Result<()> {
        debug!(path, "DELETE request");
        let resp = self
            .http
            .delete(format!("{}{}", self.base_url, path))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| ClawxError::Internal(format!("HTTP request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(ClawxError::Internal(format!(
                "API returned status {}",
                resp.status()
            )));
        }

        Ok(())
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// Return a reference to the inner HTTP client (for streaming, etc.).
    pub fn http(&self) -> &Client {
        &self.http
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_construction() {
        let client = ControlPlaneClient::new_tcp(8080, "test-token".into());
        assert_eq!(client.base_url(), "http://127.0.0.1:8080");
    }
}
