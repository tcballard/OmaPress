use anyhow::Result;
use newsletter_service::{App, api, config::Config, worker};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
#[tokio::main]
async fn main() -> Result<()> {
    let config = Arc::new(Config::from_env()?);
    let db = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    sqlx::migrate!("./migrations").run(&db).await?;
    let app = App { db, config };
    let listener = tokio::net::TcpListener::bind(
        std::env::var("NEWSLETTER_BIND").unwrap_or("127.0.0.1:8787".into()),
    )
    .await?;
    eprintln!(
        "Newsletter service listening on {}; transport={}",
        listener.local_addr()?,
        app.config.transport
    );
    let (stop, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(worker::run(app.clone(), rx));
    axum::serve(listener, api::router(app.clone()))
        .with_graceful_shutdown(async move {
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("SIGTERM handler");
            tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
            let _ = stop.send(true);
        })
        .await?;
    task.await?;
    app.db.close().await;
    Ok(())
}
