use reqwest::Client;
use sqlx::PgPool;

use crate::helpers::TestApp;

#[sqlx::test]
async fn get_session_returns_user(pool: PgPool) -> sqlx::Result<()> {
    let app = TestApp::spawn(pool).await;
    let mut client = Client::new();

    let user = app.get_user(&mut client).await;
    assert!(user.is_ok());

    Ok(())
}

#[sqlx::test]
async fn get_session_returns_same_user_if_session_valid(pool: PgPool) -> anyhow::Result<()> {
    let app = TestApp::spawn(pool).await;
    let mut client = Client::builder().cookie_store(true).build()?;

    let user1 = app.get_user(&mut client).await?;
    let user2 = app.get_user(&mut client).await?;

    assert_eq!(user1, user2);

    Ok(())
}
