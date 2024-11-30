use atrium_api::client::AtpServiceClient;
use atrium_xrpc_client::reqwest::ReqwestClient;
use secrecy::SecretString;
use skyline::client::auth::login;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = AtpServiceClient::new(ReqwestClient::new("https://bsky.social"));
    let identifier = std::env::var("BSKY_IDENTIFIER")?;
    let password = SecretString::new(std::env::var("BSKY_PASSWORD")?.into());
    if let Ok(_data) = login(client, identifier, password).await {
        println!("Successfully authenticated!");
    }

    Ok(())
}
