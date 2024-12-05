use anyhow::Result;
use bsky_sdk::agent::{config::{Config, FileStore}, BskyAgent};
use secrecy::{ExposeSecret, SecretString};

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
    pub agent: BskyAgent,
}

impl API {
    pub async fn new() -> Result<Self> {
        let agent_builder = BskyAgent::builder();
        if let Ok(config) = Config::load(&FileStore::new("config.json")).await {
            let agent = agent_builder.config(config).build().await?;
            return Ok(Self { agent } );
        } else {
            let agent = agent_builder.build().await?;
            return Ok(Self { agent } );
        }
    }

    pub async fn login(&mut self, identifier: String, password: SecretString) -> Result<()> {
        match self.agent.login(&identifier, password.expose_secret()).await {
            Ok(_) => {
                self.agent.to_config().await.save(&FileStore::new("config.json"))
                .await?;
                Ok(())
            },
            Err(e) => match e {
                _ if e.to_string().contains("Invalid password") => {
                    Err(ApiError::InvalidCredentials.into())
                }
                _ => Err(ApiError::NetworkError(e.to_string()).into()),
            },
        }
    }

    pub async fn get_timeline(
        &self,
        cursor: Option<String>,
    ) -> Result<(Vec<atrium_api::app::bsky::feed::defs::FeedViewPost>, Option<String>)> {
        let params = atrium_api::app::bsky::feed::get_timeline::ParametersData {
            algorithm: None,
            cursor,
            limit: Some(atrium_api::types::LimitedNonZeroU8::MAX),
        };
    
        match self.agent.api.app.bsky.feed.get_timeline(params.into()).await {
            Ok(response) => Ok((response.feed.clone(), response.cursor.clone())),
            Err(e) => match e {
                _ if e.to_string().contains("rate limit") => Err(ApiError::RateLimited.into()),
                _ if e.to_string().contains("unauthorized") => Err(ApiError::SessionExpired.into()),
                _ => Err(ApiError::NetworkError(e.to_string()).into()),
            },
        }
    }

    pub async fn like_post(&self, uri: &str, cid: &atrium_api::types::string::Cid) -> Result<()> {
        let record_data = atrium_api::app::bsky::feed::like::RecordData {
            created_at: atrium_api::types::string::Datetime::now(),
            subject: atrium_api::com::atproto::repo::strong_ref::MainData{
                uri: uri.try_into()?,
                cid: cid.clone(),
            }.into(),
        };
    
        self.agent.create_record(record_data).await?;
        Ok(())
    }

    pub async fn unlike_post(&self, post: &atrium_api::app::bsky::feed::defs::PostViewData) -> Result<()> {
        if let Some(viewer) = &post.viewer {
            if let Some(like) = &viewer.like {
                self.agent.delete_record(like).await?;
            }
        }
        return Ok(());
    }

    pub async fn repost(&self, uri: &str, cid: &atrium_api::types::string::Cid) -> Result<()> {
        let record_data = atrium_api::app::bsky::feed::repost::RecordData {
            created_at: atrium_api::types::string::Datetime::now(),
            subject: atrium_api::com::atproto::repo::strong_ref::MainData {
                uri: uri.try_into()?,
                cid: cid.clone(),
            }.into(),
        };
        self.agent.create_record(record_data).await?;
        Ok(())
    }

    pub async fn get_post(&self, uri: &str) -> Result<atrium_api::types::Object<atrium_api::app::bsky::feed::defs::PostViewData>> {
        let get_posts_result = self.agent.api.app.bsky.feed.get_posts(
            atrium_api::app::bsky::feed::get_posts::ParametersData {
                uris: vec![uri.to_string()],
            }.into()
        ).await;
        if let Ok(post_data) = get_posts_result {
            return Ok(post_data.data.posts[0].clone());
        } else {
            return Err(anyhow::anyhow!("Failed to get post"));
        }
    }

    // pub async fn refresh_session(&mut self) -> Result<()> {
    //     self.agent.resume_session(session)
    //     // self.agent.refresh_session().await?;
    //     Ok(())
    // }

    // pub fn get_did(&self) -> Option<String> {
    //     self.agent.get
    //     self.agent.session().map(|s| s.did().to_string())
    // }
}


// use std::path::PathBuf;
// // use std::time::{SystemTime, UNIX_EPOCH};

// use anyhow::Result;
// use atrium_api::agent::AtpAgent;
// use atrium_api::app::bsky::feed::defs::{FeedViewPost, Interaction, InteractionData};
// use atrium_api::app::bsky::feed::repost;
// use atrium_api::types::LimitedNonZeroU8;
// use atrium_api::types::{
//     string::{Did, Handle},
//     Unknown,
// };
// use chrono::Utc;
// use ipld_core::cid;
// use ipld_core::ipld::Ipld;

// use atrium_xrpc_client::reqwest::ReqwestClient;
// use secrecy::{ExposeSecret, SecretString};
// use serde::Serialize;

// // use base64::engine::general_purpose::URL_SAFE_NO_PAD;
// // use base64::Engine;
// // use serde_json::Value;

// use super::auth::FileSessionStore;

// pub const DEFAULT_PDS: &str = "public.api.bsky.app";

// pub struct SessionData {
//     pub access_jwt: String,
//     pub did: Did,
//     pub did_doc: Option<Unknown>,
//     pub email: Option<String>,
//     pub handle: Handle,
//     pub refresh_jwt: String,
// }

// #[derive(Debug, thiserror::Error)]
// pub enum ApiError {
//     #[error("Not authenticated")]
//     NotAuthenticated,

//     #[error("Session expired")]
//     SessionExpired,

//     #[error("Network error: {0}")]
//     NetworkError(String),

//     #[error("Rate limited")]
//     RateLimited,

//     #[error("Invalid credentials")]
//     InvalidCredentials,

//     #[error("Unknown error: {0}")]
//     Unknown(String),
// }

// pub struct API {
//     pub agent: AtpAgent<FileSessionStore, ReqwestClient>,
//     // client: AtpServiceClient<ReqwestClient>,
//     session_data: Option<SessionData>,
// }

// impl API {
//     pub async fn new() -> Self {
//         let agent = AtpAgent::new(
//             ReqwestClient::new("https://bsky.social"),
//             super::auth::FileSessionStore::new(PathBuf::from("session.json")),
//         );
//         Self {
//             agent,
//             session_data: None,
//         }
//     }

//     pub async fn is_authenticated(&self) -> bool {
//         if let Err(e) = self.agent.api.com.atproto.server.get_session().await {
//             print!("{:?}", e);
//             return false;
//         } else {
//             return true;
//         }
//     }

//     pub async fn refresh_session(&mut self) -> Result<()> {
//         let session = self
//             .session_data
//             .as_mut()
//             .ok_or(ApiError::NotAuthenticated)?;

//         match self.agent.api.com.atproto.server.refresh_session().await {
//             Ok(response) => {
//                 // Update session data
//                 session.access_jwt = response.data.access_jwt;
//                 session.did = response.data.did;
//                 session.did_doc = response.data.did_doc;
//                 session.handle = response.data.handle;
//                 session.refresh_jwt = response.data.refresh_jwt;
//                 Ok(())
//             }
//             Err(e) => match e {
//                 _ if e.to_string().contains("expired") => Err(ApiError::SessionExpired.into()),
//                 _ => Err(ApiError::NetworkError(e.to_string()).into()),
//             },
//         }
//     }

//     pub fn get_did(&self) -> Option<Did> {
//         if let Some(session_data) = &self.session_data {
//             return Some(session_data.did.clone());
//         } else {
//             return None;
//         }
//     }

//     pub async fn login(&mut self, identifier: String, password: SecretString) -> Result<()> {
//         match self
//             .agent
//             .login(identifier, password.expose_secret().to_string())
//             .await
//         {
//             Ok(response) => {
//                 self.session_data = Some(SessionData {
//                     access_jwt: response.data.access_jwt,
//                     did: response.data.did,
//                     did_doc: response.data.did_doc,
//                     email: response.data.email,
//                     handle: response.data.handle,
//                     refresh_jwt: response.data.refresh_jwt,
//                 });

//                 Ok(())
//             }
//             Err(e) => match e {
//                 _ if e.to_string().contains("Invalid password") => {
//                     Err(ApiError::InvalidCredentials.into())
//                 }
//                 _ => Err(ApiError::NetworkError(e.to_string()).into()),
//             },
//         }
//     }

//     pub async fn get_timeline(
//         &self,
//         cursor: Option<String>,
//     ) -> Result<(Vec<FeedViewPost>, Option<String>)> {
//         if !self.is_authenticated().await {
//             return Err(ApiError::NotAuthenticated.into());
//         }

//         let limit: LimitedNonZeroU8<100> = LimitedNonZeroU8::MAX;
//         let parameters = atrium_api::app::bsky::feed::get_timeline::Parameters {
//             data: atrium_api::app::bsky::feed::get_timeline::ParametersData {
//                 algorithm: None,
//                 cursor,
//                 limit: Some(limit),
//             },
//             extra_data: Ipld::Null,
//         };

//         match self.agent.api.app.bsky.feed.get_timeline(parameters).await {
//             Ok(response) => Ok((response.data.feed, response.data.cursor)),
//             Err(e) => match e {
//                 _ if e.to_string().contains("rate limit") => Err(ApiError::RateLimited.into()),
//                 _ if e.to_string().contains("unauthorized") => Err(ApiError::SessionExpired.into()),
//                 _ => Err(ApiError::NetworkError(e.to_string()).into()),
//             },
//         }
//     }

//     async fn create_record<D: Serialize>(
//         &self, 
//         collection: &str, 
//         record: D
//     ) -> Result<create_record::Output> {
//         let collection = Nsid::new(collection.to_string())
//             .map_err(|e| anyhow::anyhow!("Invalid NSID: {}", e))?;

//         // Convert the record to a Value first
//         let record_value = serde_json::to_value(record)?;
        
//         // Then use from_json to create the Unknown type
//         let record_unknown = Unknown::from_json(record_value)
//             .map_err(|e| anyhow::anyhow!("Failed to convert record: {}", e))?;

//         let input = create_record::Input {
//             data: create_record::InputData {
//                 collection,
//                 record: record_unknown,
//                 repo: AtIdentifier::Did(self.get_did().ok_or(ApiError::NotAuthenticated)?),
//                 rkey: None,
//                 swap_commit: None,
//                 validate: None,
//             },
//             extra_data: ipld_core::ipld::Ipld::Null,
//         };

//         Ok(self.agent.api.com.atproto.repo.create_record(input).await?)
//     }

//     pub async fn like_post(&self, uri: String, cid: atrium_api::types::string::Cid) -> Result<()> {
//         let record = atrium_api::app::bsky::feed::like::Record {
//             data: atrium_api::app::bsky::feed::like::RecordData {
//                 created_at: atrium_api::types::string::Datetime::now(),
//                 subject: Record {
//                     uri,
//                     cid,
//                 },
//             },
//             extra_data: Ipld::Null,
//         };

//         self.create_record("app.bsky.feed.like", record).await?;
//         Ok(())
//     }
    
//     pub async fn repost(&self, uri: &str, cid: &str) -> Result<()> {
//         let repost_record = repost::Record {
//             data: repost::RecordData {
//                 created_at: Utc::now(),
//                 subject: atrium_api::app::bsky::embed::record::Main {
//                     data: atrium_api::app::bsky::embed::record::MainData {
//                         record: todo!(),
//                     },
//                     extra_data: Ipld::Null,
//                 },
//             },
//             extra_data: Ipld::Null,
//         };

//         let params = atrium_api::com::atproto::repo::create_record::Parameters {
//             data: create_record::ParametersData {
//                 collection: "app.bsky.feed.repost".into(),
//                 record: repost_record.into(),
//                 repo: self.get_did().ok_or(ApiError::NotAuthenticated)?,
//                 rkey: None,
//                 validate: None,
//                 swap_commit: None,
//             },
//             extra_data: ipld_core::ipld::Ipld::Null,
//         };

//         self.agent.api.com.atproto.repo.create_record(params).await?;
//         Ok(())
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     // use std::time::{SystemTime, UNIX_EPOCH};

//     #[tokio::test]
//     async fn test_auth_login_success() -> Result<()> {
//         let mut api = API::new().await;
//         let identifier = std::env::var("BSKY_IDENTIFIER")?;
//         let password = SecretString::new(std::env::var("BSKY_PASSWORD")?.into());

//         println!(
//             "Before login: authenticated = {:?}",
//             api.is_authenticated().await
//         );
//         api.login(identifier, password).await?;
//         println!(
//             "After login: authenticated = {:?}",
//             api.is_authenticated().await
//         );
//         println!("Session data present: {}", api.session_data.is_some());

//         assert!(api.is_authenticated().await);
//         Ok(())
//     }

//     // fn create_test_token(expires_in_seconds: i64) -> String {
//     //     let now = SystemTime::now()
//     //         .duration_since(UNIX_EPOCH)
//     //         .unwrap()
//     //         .as_secs() as i64;

//     //     let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
//     //     let claims = format!(r#"{{"exp":{}}}"#, now + expires_in_seconds);
//     //     let claims_b64 = URL_SAFE_NO_PAD.encode(claims);

//     //     format!("{}.{}.test_sig", header, claims_b64)
//     // }
// }
