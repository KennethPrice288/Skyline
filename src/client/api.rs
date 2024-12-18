use anyhow::Result;
use bsky_sdk::agent::{config::{Config, FileStore}, BskyAgent};
use secrecy::{ExposeSecret, SecretString};

const CONFIG_PATH: &str = "config.json";

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

#[derive(Clone)]
pub struct API {
    pub agent: BskyAgent,
}

impl API {
    pub async fn new() -> Result<Self> {
        let agent_builder = BskyAgent::builder();
        if let Ok(config) = Config::load(&FileStore::new(CONFIG_PATH)).await {
            if let Ok(agent) = agent_builder.config(config).build().await {
                return Ok(Self { agent } );
            } else {
                let agent_builder = BskyAgent::builder();
                let agent = agent_builder.build().await?;
                return Ok(Self { agent } );
            }
        } else {
            let agent = agent_builder.build().await?;
            return Ok(Self { agent } );
        }
    }

    pub async fn login(&mut self, identifier: String, password: SecretString) -> Result<()> {
        match self.agent.login(&identifier, password.expose_secret()).await {
            Ok(_) => {
                self.agent.to_config().await.save(&FileStore::new(CONFIG_PATH))
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
    
    pub async fn logout(&mut self) -> Result<()> {
        // Clear the stored session file
        tokio::fs::remove_file(CONFIG_PATH).await.ok(); // Use ok() to ignore if file doesn't exist
        
        // Create a fresh agent
        let agent_builder = BskyAgent::builder();
        self.agent = agent_builder.build().await?;
        
        Ok(())
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
        match self.agent.create_record(record_data).await {
            Ok(_) => {},
            Err(e) => {log::info!("error reposting: {:?}", e)}
        }
        Ok(())
    }

    pub async fn unrepost(&self, post: &atrium_api::app::bsky::feed::defs::PostViewData) -> Result<()> {
        if let Some(viewer) = &post.viewer {
            if let Some(repost) = &viewer.repost {
                self.agent.delete_record(repost).await?;
            }
        }
        return Ok(());
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

    pub async fn refresh_session(&mut self) -> Result<()> {
        if let Some(session) = self.agent.get_session().await {
            self.agent.resume_session(session).await?;
        } else {
            return Err(anyhow::anyhow!("could not resume session, session may not exist"));
        }
        Ok(())
    }

    pub async fn follow_actor(&mut self, did: atrium_api::types::string::Did) -> Result<()> {
        let record_data = atrium_api::app::bsky::graph::follow::RecordData {
            created_at: atrium_api::types::string::Datetime::now(),
            subject: did.clone(),
        };
        match self.agent.create_record(record_data).await {
            Ok(_) => {log::info!("Followed did: {:?}", did); Ok(())},
            Err(e) => {log::error!("Failed to follow did: {:?} with error: {}", did, e); Err(e.into())},
        }
    }

    pub async fn unfollow_actor(&mut self, did: &atrium_api::types::string::Did) -> Result<()> {
        // First get the profile to find the follow record URI
        let params = atrium_api::app::bsky::actor::get_profile::ParametersData {
            actor: atrium_api::types::string::AtIdentifier::Did(did.clone())
        }.into();
        
        if let Ok(profile) = self.agent.api.app.bsky.actor.get_profile(params).await {
            if let Some(viewer) = &profile.viewer {
                if let Some(follow) = &viewer.following {
                    // If we have the follow record URI, delete it
                    self.agent.delete_record(&follow).await?;
                    log::info!("Unfollowed did: {:?}", did);
                    return Ok(());
                }
            }
        }
        
        Err(anyhow::anyhow!("Could not find follow record to delete"))
    }

    pub async fn create_post(&self, text: String, reply_to: Option<String>) -> Result<()> {
        let mut record = atrium_api::app::bsky::feed::post::RecordData {
            text,
            created_at: atrium_api::types::string::Datetime::now(),
            reply: None,
            embed: None,
            langs: None,
            labels: None,
            tags: None,
            facets: None,
            entities: None,
        };

        // If this is a reply, set up the reply reference
        if let Some(reply_uri) = reply_to {
            // First get the post we're replying to
            let parent_post = self.get_post(&reply_uri).await?;
            
            record.reply = Some(atrium_api::app::bsky::feed::post::ReplyRefData {
                root: atrium_api::com::atproto::repo::strong_ref::MainData {
                    uri: reply_uri.clone().try_into()?,
                    cid: parent_post.cid.clone(),
                }.into(),
                parent: atrium_api::com::atproto::repo::strong_ref::MainData {
                    uri: reply_uri.try_into()?,
                    cid: parent_post.cid.clone(),
                }.into(),
            }.into());
        }

        match self.agent.create_record(record).await {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Failed to create post: {}", e))
        }
    }
    pub async fn delete_post(&self, uri: &str) -> Result<()> {
        let repo_uri: String = uri.try_into()?;
        
        match self.agent.delete_record(&repo_uri).await {
            Ok(_) => {
                log::info!("Successfully deleted post: {}", uri);
                Ok(())
            },
            Err(e) => {
                log::error!("Failed to delete post: {}", e);
                Err(ApiError::NetworkError(e.to_string()).into())
            }
        }
    }
}
