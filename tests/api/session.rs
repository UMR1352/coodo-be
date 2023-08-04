use reqwest::Client;

use crate::helpers::TestApp;

#[tokio::test]
async fn get_session_returns_user() -> anyhow::Result<()> {
    let app = TestApp::spawn().await;
    let mut client = Client::new();

    let user = app.get_user(&mut client).await;
    assert!(user.is_ok());

    Ok(())
}

#[tokio::test]
async fn get_session_returns_same_user_if_session_valid() -> anyhow::Result<()> {
    let app = TestApp::spawn().await;
    let mut client = Client::builder().cookie_store(true).build()?;

    let user1 = app.get_user(&mut client).await?;
    let user2 = app.get_user(&mut client).await?;

    assert_eq!(user1, user2);

    Ok(())
}
