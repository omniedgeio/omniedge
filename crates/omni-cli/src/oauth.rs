use anyhow::Result;
use log::info;
use omni_api::{types::AuthResp, ApiClient, AuthService};
use omni_core::CliConfig;

/// Attempt to refresh the token using the refresh_token
async fn try_refresh_token(base_url: &str, refresh_token: &str) -> Result<AuthResp> {
    info!("Attempting to refresh access token...");
    let client = ApiClient::new(base_url.to_string(), None);
    let auth_service = AuthService::new(&client);
    auth_service.refresh_token(refresh_token).await
}

pub async fn ensure_auth(base_url: &str, config: &mut CliConfig) -> Result<AuthResp> {
    // Check if we have a saved auth response
    if let Some(auth) = &config.auth_response {
        // Check if token is expired or about to expire
        if config.is_token_expired() {
            info!("Token expired or expiring soon, attempting refresh...");

            // Try to refresh using the refresh_token
            if !auth.refresh_token.is_empty() {
                match try_refresh_token(base_url, &auth.refresh_token).await {
                    Ok(mut new_auth) => {
                        // Preserve refresh_token if new response doesn't include one
                        if new_auth.refresh_token.is_empty() {
                            new_auth.refresh_token = auth.refresh_token.clone();
                        }
                        // Ensure token field is set
                        if new_auth.token.is_empty() {
                            new_auth.token = new_auth.access_token.clone();
                        }

                        info!("Token refreshed successfully");
                        config.set_auth_response(new_auth.clone());
                        config.save()?;
                        return Ok(new_auth);
                    }
                    Err(e) => {
                        info!(
                            "Token refresh failed: {}. Will require re-authentication.",
                            e
                        );
                        // Clear the expired auth and fall through to device flow
                        config.auth_response = None;
                        config.token_obtained_at = None;
                    }
                }
            } else {
                info!("No refresh token available, will require re-authentication.");
                config.auth_response = None;
                config.token_obtained_at = None;
            }
        } else {
            // Token is still valid
            return Ok(auth.clone());
        }
    }

    println!("No saved login found. Starting device flow...");
    let client = ApiClient::new(base_url.to_string(), None);
    let auth_service = AuthService::new(&client);

    let dr = auth_service
        .device_flow_init("omniedge-cli", "openid profile email offline_access")
        .await?;
    println!("\nPlease visit: {}", dr.verification_uri);
    println!("And enter the code: {}\n", dr.user_code);

    let mut interval = dr.interval as u64;
    if interval == 0 {
        interval = 5;
    }

    let mut auth = loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
        match auth_service
            .device_flow_token("omniedge-cli", &dr.device_code)
            .await
        {
            Ok(mut token) => {
                if token.token.is_empty() {
                    token.token = token.access_token.clone();
                }
                break token;
            }
            Err(e) => {
                let err_str = e.to_string().to_lowercase();
                if err_str.contains("authorization_pending") || err_str.contains("slow_down") {
                    if err_str.contains("slow_down") {
                        interval += 5;
                    }
                    continue;
                }
                return Err(e);
            }
        }
    };

    // Ensure token field is set
    if auth.token.is_empty() {
        auth.token = auth.access_token.clone();
    }

    config.set_auth_response(auth.clone());
    config.save()?;
    Ok(auth)
}

pub async fn login_with_security_key(
    base_url: &str,
    security_key: &str,
    config: &mut CliConfig,
) -> Result<AuthResp> {
    println!("Logging in with security key...");
    let client = ApiClient::new(base_url.to_string(), None);
    let auth_service = AuthService::new(&client);

    let auth = auth_service.login_with_security_key(security_key).await?;

    config.set_auth_response(auth.clone());
    config.save()?;
    println!("Successfully logged in.");
    Ok(auth)
}
