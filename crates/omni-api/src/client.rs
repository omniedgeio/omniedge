use crate::types::{ErrorResponse, SuccessResponse};
use anyhow::{anyhow, Result};
use log::{debug, error, info};
use reqwest::{Client as HttpClient, RequestBuilder};
use serde::de::DeserializeOwned;

#[derive(Clone)]
pub struct ApiClient {
    pub client: HttpClient,
    pub base_url: String,
    pub token: Option<String>,
}

impl ApiClient {
    pub fn new(base_url: String, token: Option<String>) -> Self {
        Self {
            client: HttpClient::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
        }
    }

    /// Send a request, logging errors at ERROR level
    pub async fn send<T>(&self, builder: RequestBuilder) -> Result<T>
    where
        T: DeserializeOwned,
    {
        self.send_internal(builder, false).await
    }

    /// Send a request without logging errors (for polling/expected failures)
    pub async fn send_quiet<T>(&self, builder: RequestBuilder) -> Result<T>
    where
        T: DeserializeOwned,
    {
        self.send_internal(builder, true).await
    }

    async fn send_internal<T>(&self, builder: RequestBuilder, quiet: bool) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let mut builder = builder;
        if let Some(token) = &self.token {
            if !token.is_empty() {
                let auth_header = if token.to_lowercase().starts_with("bearer ") {
                    token.clone()
                } else {
                    format!("Bearer {}", token)
                };
                builder = builder.header("Authorization", auth_header);
            }
        }

        let resp = builder.send().await?;
        let status = resp.status();
        let url = resp.url().clone();
        if quiet {
            debug!("API Request: {} {}", url, status);
        } else {
            info!("API Request: {} {}", url, status);
        }
        let body_bytes = resp.bytes().await?;

        if status.is_success() {
            // First try parsing with SuccessResponse wrapper
            if let Ok(data) = serde_json::from_slice::<SuccessResponse<T>>(&body_bytes) {
                return Ok(data.data);
            }

            // Fallback: try parsing T directly
            match serde_json::from_slice::<T>(&body_bytes) {
                Ok(data) => Ok(data),
                Err(e) => {
                    let body_str = String::from_utf8_lossy(&body_bytes);
                    error!(
                        "Failed to parse successful response from {}: {}\nBody: {}",
                        url, e, body_str
                    );
                    Err(anyhow!(
                        "Failed to parse response (tried wrapped and unwrapped): {}\nBody: {}",
                        e,
                        body_str
                    ))
                }
            }
        } else {
            let body_str = String::from_utf8_lossy(&body_bytes);
            if !quiet {
                error!(
                    "API Error response from {}: {}\nBody: {}",
                    url, status, body_str
                );
            } else {
                debug!(
                    "API Error response from {}: {}\nBody: {}",
                    url, status, body_str
                );
            }
            let error_err: ErrorResponse =
                serde_json::from_slice(&body_bytes).unwrap_or_else(|_| ErrorResponse {
                    code: None,
                    message: Some(body_str.to_string()), // Fallback to raw body as message
                    errors: None,
                    error: None,
                    error_description: None,
                });

            let msg = error_err
                .error
                .clone()
                .or(error_err.message.clone())
                .or(error_err.error_description.clone())
                .unwrap_or_else(|| "Unknown error".to_string());

            Err(anyhow!("API Error ({}): {}", status, msg))
        }
    }

    pub fn build_url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    pub fn ws_url(&self, path: &str) -> String {
        self.build_url(path)
            .replace("http://", "ws://")
            .replace("https://", "wss://")
    }

    pub fn post(&self, path: &str) -> RequestBuilder {
        let url = self.build_url(path);
        info!("API POST: {}", url);
        self.client.post(url)
    }

    pub fn get(&self, path: &str) -> RequestBuilder {
        let url = self.build_url(path);
        info!("API GET: {}", url);
        self.client.get(url)
    }

    pub fn put(&self, path: &str) -> RequestBuilder {
        let url = self.build_url(path);
        info!("API PUT: {}", url);
        self.client.put(url)
    }
}
