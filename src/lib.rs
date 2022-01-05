pub mod broker;
pub mod config;
pub mod kafka;
pub mod raft;

use crate::broker::JosefineBroker;
use anyhow::Result;
use crate::raft::client::RaftClient;
use futures::FutureExt;
use crate::config::JosefineConfig;

use crate::raft::JosefineRaft;

#[macro_use]
extern crate serde_derive;

pub async fn josefine_with_config(
    config: JosefineConfig,
    shutdown: (
        tokio::sync::broadcast::Sender<()>,
        tokio::sync::broadcast::Receiver<()>,
    ),
) -> Result<()> {
    run(config, shutdown).await?;
    Ok(())
}

pub async fn josefine<P: AsRef<std::path::Path>>(
    config_path: P,
    shutdown: (
        tokio::sync::broadcast::Sender<()>,
        tokio::sync::broadcast::Receiver<()>,
    ),
) -> Result<()> {
    let config = config::config(config_path);
    run(config, shutdown).await?;
    Ok(())
}

#[tracing::instrument]
pub async fn run(config: JosefineConfig,     shutdown: (
    tokio::sync::broadcast::Sender<()>,
    tokio::sync::broadcast::Receiver<()>,
),) -> Result<()> {
    tracing::info!("starting");
    let db = sled::open(&config.broker.state_file).unwrap();

    let (client_tx, client_rx) = tokio::sync::mpsc::unbounded_channel();
    let client = RaftClient::new(client_tx);
    let josefine_broker = JosefineBroker::new(config.broker);
    let broker = broker::state::Store::new(db);
    let (task, b) = josefine_broker
        .run(
            client,
            broker.clone(),
            (shutdown.0.clone(), shutdown.0.subscribe()),
        )
        .remote_handle();
    tokio::spawn(task);

    let raft = JosefineRaft::new(config.raft);
    let (task, raft) = raft
        .run(
            crate::broker::fsm::JosefineFsm::new(broker),
            client_rx,
            (shutdown.0.clone(), shutdown.0.subscribe()),
        )
        .remote_handle();
    tokio::spawn(task);

    let (task, shutdown_notifier) = async move {
        let mut rx = shutdown.0.subscribe();
        let _ = rx.recv().await?;
        tracing::info!("received shutdown signal");
        Ok(())
    }.remote_handle();
    tokio::spawn(task);

    let (_, _, _) = tokio::try_join!(b, raft, shutdown_notifier)?;
    Ok(())
}
