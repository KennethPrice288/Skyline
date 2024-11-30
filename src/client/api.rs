use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use atrium_api::app::bsky::feed::defs::FeedViewPost;
use atrium_api::client::AtpServiceClient;
use atrium_api::types::LimitedNonZeroU8;
use atrium_api::types::{
    string::{Did, Handle},
    Unknown,
};
use ipld_core::ipld::Ipld;

use atrium_xrpc_client::reqwest::ReqwestClient;
use secrecy::{ExposeSecret, SecretString};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::Value;


pub const DEFAULT_PDS: &str = "https://bsky.social";

pub struct SessionData {
    pub access_jwt: String,
    pub did: Did,
    pub did_doc: Option<Unknown>,
    pub email: Option<String>,
    pub handle: Handle,
    pub refresh_jwt: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Not authenticated")]
    NotAuthenticated,
    
    #[error("Session expired")]
    SessionExpired,
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Rate limited")]
    RateLimited,
    
    #[error("Invalid credentials")]
    InvalidCredentials,
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}


pub struct API {
    client: AtpServiceClient<ReqwestClient>,
    session_data: Option<SessionData>,
}

fn is_token_expired(token: &str) -> bool {
    // Split JWT into parts
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        println!("Invalid JWT format");
        return true;
    }

    // Decode the claims (middle part)
    match URL_SAFE_NO_PAD.decode(parts[1]) {
        Ok(claims_json) => {
            match serde_json::from_slice::<Value>(&claims_json) {
                Ok(claims) => {
                    match claims["exp"].as_i64() {
                        Some(exp) => {
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs() as i64;
                            println!("Token expires at: {}, current time: {}", exp, now);
                            exp <= now
                        }
                        None => {
                            println!("No exp claim found");
                            true
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to parse claims JSON: {}", e);
                    true
                }
            }
        }
        Err(e) => {
            println!("Failed to decode base64: {}", e);
            true
        }
    }
}



impl API {
    pub async fn new() -> Self {
        Self {
            client: AtpServiceClient::new(ReqwestClient::new(DEFAULT_PDS)),
            session_data: None,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        if let Some(session) = &self.session_data {
            !is_token_expired(&session.access_jwt)
        } else {
            false
        }
    }

    pub async fn refresh_session(&mut self) -> Result<()> {
        let session = self.session_data.as_mut()
            .ok_or(ApiError::NotAuthenticated)?;
    
        match self.client
            .service
            .com
            .atproto
            .server
            .refresh_session()
            .await {
                Ok(response) => {
                    // Update session data
                    session.access_jwt = response.data.access_jwt;
                    session.did = response.data.did;
                    session.did_doc = response.data.did_doc;
                    session.handle = response.data.handle;
                    session.refresh_jwt = response.data.refresh_jwt;
                    Ok(())
                },
                Err(e) => {
                    match e {
                        _ if e.to_string().contains("expired") => 
                            Err(ApiError::SessionExpired.into()),
                        _ => Err(ApiError::NetworkError(e.to_string()).into())
                    }
                }
        }
    }
    

    pub fn get_did(&self) -> Option<Did> {
        if let Some(session_data) = &self.session_data {
            return Some(session_data.did.clone());
        } else {
            return None;
        }
    }

    pub async fn login(&mut self, identifier: String, password: SecretString) -> Result<()> {
        match self.client
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
            .await {
                Ok(response) => {
                    self.session_data = Some(SessionData {
                        access_jwt: response.data.access_jwt,
                        did: response.data.did,
                        did_doc: response.data.did_doc,
                        email: response.data.email,
                        handle: response.data.handle,
                        refresh_jwt: response.data.refresh_jwt,
                    });
                    Ok(())
                },
                Err(e) => {
                    match e {
                        _ if e.to_string().contains("Invalid password") => 
                            Err(ApiError::InvalidCredentials.into()),
                        _ => Err(ApiError::NetworkError(e.to_string()).into())
                    }
                }
            }
    }


    pub async fn get_timeline(&self, cursor: Option<String>) -> Result<(Vec<FeedViewPost>, Option<String>)> {
        if !self.is_authenticated() {
            return Err(ApiError::NotAuthenticated.into());
        }

        let limit: LimitedNonZeroU8<100> = LimitedNonZeroU8::MAX;
        let parameters = atrium_api::app::bsky::feed::get_timeline::Parameters {
            data: atrium_api::app::bsky::feed::get_timeline::ParametersData {
                algorithm: None,
                cursor,
                limit: Some(limit),
            },
            extra_data: Ipld::Null,
        };

        match self.client.service.app.bsky.feed.get_timeline(parameters).await {
            Ok(response) => Ok((response.data.feed, response.data.cursor)),
            Err(e) => {
                match e {
                    _ if e.to_string().contains("rate limit") => Err(ApiError::RateLimited.into()),
                    _ if e.to_string().contains("unauthorized") => Err(ApiError::SessionExpired.into()),
                    _ => Err(ApiError::NetworkError(e.to_string()).into())
                }
            }
        }
    }


}


#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn test_auth_login_success() -> Result<()> {
        let mut api = API::new().await;
        let identifier = std::env::var("BSKY_IDENTIFIER")?;
        let password = SecretString::new(std::env::var("BSKY_PASSWORD")?.into());
        
        println!("Before login: authenticated = {:?}", api.is_authenticated());
        api.login(identifier, password).await?;
        println!("After login: authenticated = {:?}", api.is_authenticated());
        println!("Session data present: {}", api.session_data.is_some());
        
        assert!(api.is_authenticated());
        Ok(())
    }

    fn create_test_token(expires_in_seconds: i64) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
        let claims = format!(r#"{{"exp":{}}}"#, now + expires_in_seconds);
        let claims_b64 = URL_SAFE_NO_PAD.encode(claims);
        
        format!("{}.{}.test_sig", header, claims_b64)
    }
    

    #[test]
    fn test_token_expiration() {
        assert!(is_token_expired(&create_test_token(-60))); // expired
        assert!(!is_token_expired(&create_test_token(60))); // valid
    }
    
}
