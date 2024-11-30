use anyhow::{anyhow, Result};
use atrium_api::client::AtpServiceClient;
use atrium_api::types::{
    string::{Did, Handle},
    Unknown,
};
use atrium_xrpc_client::reqwest::ReqwestClient;
use secrecy::{ExposeSecret, SecretString};

pub const DEFAULT_PDS: &str = "https://bsky.social";

pub struct SessionData {
    pub access_jwt: String,
    pub did: Did,
    pub did_doc: Option<Unknown>,
    pub email: Option<String>,
    pub handle: Handle,
    pub refresh_jwt: String,
}

pub struct API {
    client: Option<AtpServiceClient<ReqwestClient>>,
    session_data: Option<SessionData>,
}

impl API {
    pub fn new() -> Self {
        Self {
            client: None,
            session_data: None,
        }
    }

    pub fn new_client(&mut self) {
        self.client = Some(AtpServiceClient::new(ReqwestClient::new(DEFAULT_PDS)));
    }

    pub fn is_authenticated(&self) -> bool {
        if self.session_data.is_none() {
            return false;
        } else {
        }
    }

    pub async fn refresh_session(&mut self) -> Result<()> {
        let data = self
            .client
            .as_ref()
            .ok_or(anyhow!("No client"))?
            .service
            .com
            .atproto
            .server
            .refresh_session()
            .await?
            .data;

        self.session_data = Some(SessionData {
            access_jwt: data.access_jwt,
            did: data.did,
            did_doc: data.did_doc,
            email: data.email,
            handle: data.handle,
            refresh_jwt: data.refresh_jwt,
        });
        Ok(())
    }

    pub fn get_did(&self) -> Option<Did> {
        if let Some(session_data) = &self.session_data {
            return Some(session_data.did.clone());
        } else {
            return None;
        }
    }

    pub async fn login(&mut self, identifier: String, password: SecretString) -> Result<()> {
        let data = self
            .client
            .as_ref()
            .ok_or(anyhow!("No client"))?
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
            .data;
        self.session_data = Some(SessionData {
            access_jwt: data.access_jwt,
            did: data.did,
            did_doc: data.did_doc,
            email: data.email,
            handle: data.handle,
            refresh_jwt: data.refresh_jwt,
        });
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_auth_login_success() -> Result<()> {
        let mut api = API::new();
        api.new_client();
        let identifier = std::env::var("BSKY_IDENTIFIER")?;
        let password = SecretString::new(std::env::var("BSKY_PASSWORD")?.into());
        // Test successful login sets up client and session
        api.login(identifier, password).await
    }
}
