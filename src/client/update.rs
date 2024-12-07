use std::time::Duration;
use anyhow::Result;
use atrium_api::app::bsky::notification::list_notifications::NotificationData;
use futures_util::StreamExt;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_tungstenite::{connect_async, tungstenite::{handshake::client::generate_key, Message}};
use serde::Deserialize;
use log::error;
use ipld_core::ipld::Ipld;

#[derive(Debug, Deserialize)]
#[serde(tag = "t")] 
#[allow(dead_code)]
enum SubscriptionMessage {
    #[serde(rename = "commit")]
    Commit(RepoCommit),
    #[serde(rename = "handle")]
    Handle(HandleChange),
    #[serde(rename = "tombstone")] 
    Tombstone(RecordDelete),
    #[serde(rename = "migrate")]
    Migrate(Migration),
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RepoCommit {
    #[serde(rename = "#c")]
    collection: String,
    commit: CommitInfo,
    repo: String,
    time: String,
    blocks: Vec<Block>
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HandleChange {
    did: String,
    handle: String,
    time: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RecordDelete {
    uri: String,
    time: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Migration {
    did: String,
    migrated_to: String,
    time: String,
}

#[derive(Debug, Deserialize)]
struct Block {
    cid: String,
    #[serde(rename = "val")]
    value: Ipld,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CommitInfo {
    #[serde(rename = "seq")]
    sequence: i64,
    #[serde(rename = "rebase")]
    is_rebase: bool,
    #[serde(rename = "tooBig")]
    too_big: bool,
    ops: Vec<Operation>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Operation {
    action: String,  // "create", "update", "delete"
    path: String,
    #[serde(rename = "cid")]
    content_id: String,
}
// Represents different types of real-time updates
#[derive(Debug, Clone)]
pub enum UpdateEvent {
    Notification {
        uri: String,
    },
    ConnectionStatus(ConnectionStatus),
}

#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Reconnecting,
}

pub struct UpdateManager {
    sender: mpsc::Sender<UpdateEvent>,
    receiver: mpsc::Receiver<UpdateEvent>,
    ws_task: Option<JoinHandle<()>>,
    reconnect_interval: Duration,
    service_url: String,
}

impl UpdateManager {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(100);
        Self {
            sender,
            receiver,
            ws_task: None,
            reconnect_interval: Duration::from_secs(5),
            service_url: "wss://bsky.network/xrpc/com.atproto.sync.subscribeRepos".to_string(),
        }
    }

    pub async fn start(&mut self, auth_jwt: String) -> Result<()> {
        let sender = self.sender.clone();
        let service_url = self.service_url.clone();
        let reconnect_interval = self.reconnect_interval;

        let task = tokio::spawn(async move {
            loop {
                match Self::run_subscription(&service_url, &auth_jwt, &sender).await {
                    Ok(_) => {
                        error!("WebSocket connection closed normally");
                    }
                    Err(e) => {
                        error!("WebSocket error: {:?}", e);
                    }
                }

                // Notify about disconnection
                let _ = sender.send(UpdateEvent::ConnectionStatus(ConnectionStatus::Disconnected)).await;
                
                // Wait before reconnecting
                tokio::time::sleep(reconnect_interval).await;
                
                // Notify about reconnection attempt
                let _ = sender.send(UpdateEvent::ConnectionStatus(ConnectionStatus::Reconnecting)).await;
            }
        });

        self.ws_task = Some(task);
        Ok(())
    }

    async fn run_subscription(
        service_url: &str,
        auth_jwt: &str,
        sender: &mpsc::Sender<UpdateEvent>,
    ) -> Result<()> {
        // Parse URL to get host
        let url = url::Url::parse(service_url)?;
        let host = url.host_str().ok_or_else(|| anyhow::anyhow!("Missing host in URL"))?;
    
        // Create request with all required headers
        let request = http::Request::builder()
            .uri(service_url)
            .header("Host", host)
            .header("Authorization", format!("Bearer {}", auth_jwt))
            .header("Upgrade", "websocket")
            .header("Connection", "Upgrade")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", generate_key())
            .body(())?;
    
        // Connect to WebSocket
        let (ws_stream, _) = connect_async(request).await?;
        let (_, mut read) = ws_stream.split();

        // Send successful connection event
        sender.send(UpdateEvent::ConnectionStatus(ConnectionStatus::Connected)).await?;

        // Handle incoming messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    match Self::parse_update(&text) {
                        Ok(Some(event)) => {
                            if let Err(e) = sender.send(event).await {
                                log::error!("Failed to send update event: {:?}", e);
                                break;
                            }
                        }
                        Ok(None) => continue,
                        Err(e) => {
                            log::error!("Failed to parse update: {:?}", e);
                            continue;
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    log::info!("WebSocket connection closed by server");
                    break;
                }
                Err(e) => {
                    log::error!("WebSocket error: {:?}", e);
                    break;
                }
                _ => {} // Ignore other message types
            }
        }

        Ok(())
    }
    
    fn parse_update(text: &str) -> Result<Option<UpdateEvent>> {
        let message: SubscriptionMessage = serde_json::from_str(text)?;

        match message {
            SubscriptionMessage::Commit(commit) => {
                // Only care about notification collection
                if !commit.collection.starts_with("app.bsky.notification") {
                    return Ok(None);
                }

                // Process each operation in the commit
                for op in commit.commit.ops {
                    // Find the corresponding block for this operation
                    if let Some(block) = commit.blocks.iter().find(|b| b.cid == op.content_id) {
                        // Try to parse notification data from the block
                        if let Ok(_notification) = serde_json::from_value::<NotificationData>(
                            serde_json::to_value(&block.value)?
                        ) {
                            return Ok(Some(UpdateEvent::Notification {
                                uri: format!("at://{}/app.bsky.notification/{}", 
                                    commit.repo,
                                    op.path.split('/').last().unwrap_or_default()
                                ),
                            }));
                        }
                    }
                }
            }
            SubscriptionMessage::Handle(_) => {
                // Could track handle changes if needed
            }
            SubscriptionMessage::Tombstone(_delete) => {
                // Could track deleted notifications if needed
            }
            SubscriptionMessage::Migrate(_) => {
                // Could handle DID migrations if needed
            }
        }

        Ok(None)
    }

    pub fn try_recv(&mut self) -> Option<UpdateEvent> {
        self.receiver.try_recv().ok()
    }

    pub async fn stop(&mut self) {
        if let Some(task) = self.ws_task.take() {
            task.abort();
        }
    }
}

impl Drop for UpdateManager {
    fn drop(&mut self) {
        if let Some(task) = self.ws_task.take() {
            task.abort();
        }
    }
}
