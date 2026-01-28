use crate::client::ApiClient;
use crate::types::{AuthResp, DeviceCodeResp, ProfileResponse, SessionResponse};
use anyhow::Result;
use serde_json::json;

pub struct AuthService<'a> {
    pub client: &'a ApiClient,
}

impl<'a> AuthService<'a> {
    pub fn new(client: &'a ApiClient) -> Self {
        Self { client }
    }

    pub async fn login_with_password(&self, email: &str, password: &str) -> Result<AuthResp> {
        let builder = self.client.post("/auth/login/password").json(&json!({
            "email": email,
            "password": password,
        }));
        self.client.send(builder).await
    }

    pub async fn login_with_security_key(&self, key: &str) -> Result<AuthResp> {
        let builder = self.client.post("/auth/login/security-key").json(&json!({
            "key": key,
        }));
        self.client.send(builder).await
    }

    pub async fn generate_session(&self) -> Result<SessionResponse> {
        let builder = self.client.get("/auth/login/session");
        self.client.send(builder).await
    }

    pub async fn device_flow_init(&self, client_id: &str, scope: &str) -> Result<DeviceCodeResp> {
        let builder = self.client.post("/oauth/device/code").json(&json!({
            "client_id": client_id,
            "scope": scope,
        }));
        self.client.send(builder).await
    }

    pub async fn device_flow_token(&self, client_id: &str, device_code: &str) -> Result<AuthResp> {
        let builder = self.client.post("/oauth/token").json(&json!({
            "client_id": client_id,
            "device_code": device_code,
            "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
        }));
        self.client.send(builder).await
    }

    pub async fn refresh_token(&self, refresh_token: &str) -> Result<AuthResp> {
        let builder = self.client.post("/oauth/token").json(&json!({
            "refresh_token": refresh_token,
            "grant_type": "refresh_token",
            "client_id": "omniedge-desktop"
        }));
        self.client.send(builder).await
    }

    pub async fn me(&self) -> Result<ProfileResponse> {
        let builder = self.client.get("/profile");
        self.client.send(builder).await
    }
}
