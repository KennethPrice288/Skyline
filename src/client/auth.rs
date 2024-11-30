use atrium_api::agent::store::SessionStore;
use atrium_api::agent::Session;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

#[derive(Serialize, Deserialize)]
struct SessionData {
    session: Session,
}

pub struct FileSessionStore {
    file_path: PathBuf,
}

impl FileSessionStore {
    pub fn new(file_path: PathBuf) -> Self {
        FileSessionStore { file_path }
    }
}

#[derive(thiserror::Error, Debug)]
enum FileSessionStoreError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] serde_json::Error),
}

impl SessionStore for FileSessionStore {
    async fn get_session(&self) -> Option<Session> {
        match fs::read_to_string(&self.file_path).await {
            Ok(contents) => {
                let session_data: SessionData = serde_json::from_str(&contents).ok()?;
                Some(session_data.session)
            }
            Err(_) => None,
        }
    }

    async fn set_session(&self, session: Session) -> () {
        let session_data = SessionData { session };
        let contents = serde_json::to_string(&session_data).unwrap();
        if let Err(err) = fs::write(&self.file_path, contents).await {
            println!("Error saving session data: {:?}", err);
        }
    }

    async fn clear_session(&self) -> () {
        if let Err(err) = fs::remove_file(&self.file_path).await {
            println!("Error clearing session data: {:?}", err);
        }
    }
}
