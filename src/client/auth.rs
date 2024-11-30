use anyhow::Result;
use atrium_api::client::AtpServiceClient;
use atrium_api::com::atproto::server::create_session::OutputData;
use atrium_xrpc_client::reqwest::ReqwestClient;
use secrecy::{ExposeSecret, SecretString};

pub async fn login(
    client: AtpServiceClient<ReqwestClient>,
    identifier: String,
    password: SecretString,
) -> Result<OutputData> {
    Ok(client
        .service
        .com
        .atproto
        .server
        .create_session(
            atrium_api::com::atproto::server::create_session::InputData {
                auth_factor_token: None,
                identifier: identifier.clone(),
                password: password.expose_secret().to_string().clone(),
            }
            .into(),
        )
        .await?
        .data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_auth_login_success() -> Result<()> {
        let client = AtpServiceClient::new(ReqwestClient::new("https://bsky.social"));
        let identifier = std::env::var("BSKY_IDENTIFIER")?;
        let password = SecretString::new(std::env::var("BSKY_PASSWORD")?.into());
        // Test successful login sets up client and session
        let result = login(client, identifier, password).await;
        assert!(result.is_ok());
        Ok(())
    }
}
