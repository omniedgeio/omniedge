use anyhow::Result;
use omni_api::{types::AuthResp, ApiClient, AuthService};
use omni_core::CliConfig;

pub async fn ensure_auth(base_url: &str, config: &mut CliConfig) -> Result<AuthResp> {
    if let Some(auth) = &config.auth_response {
        return Ok(auth.clone());
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

    let auth = loop {
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

    config.auth_response = Some(auth.clone());
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

    config.auth_response = Some(auth.clone());
    config.save()?;
    println!("Successfully logged in.");
    Ok(auth)
}
