//! Live notification streaming via SurrealDB LIVE SELECT + tokio broadcast

use std::sync::OnceLock;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub struct NotificationEvent {
    pub person_id: String,
    pub action: String,
}

static SENDER: OnceLock<broadcast::Sender<NotificationEvent>> = OnceLock::new();

pub fn subscribe() -> broadcast::Receiver<NotificationEvent> {
    SENDER
        .get()
        .expect("notification stream not initialized")
        .subscribe()
}

pub async fn init() {
    let (tx, _) = broadcast::channel::<NotificationEvent>(256);
    SENDER
        .set(tx.clone())
        .expect("notification stream already initialized");

    tokio::spawn(async move {
        // Small delay to let DB fully initialize
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        loop {
            match run_live_query(tx.clone()).await {
                Ok(()) => {
                    warn!("Notification LIVE stream ended, restarting in 5s");
                }
                Err(e) => {
                    error!("Notification LIVE stream error: {}, restarting in 5s", e);
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
}

async fn run_live_query(
    tx: broadcast::Sender<NotificationEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use futures::StreamExt;

    info!("Connecting LIVE SELECT on notification table...");

    let stream_result = crate::db::DB.select("notification").live().await;

    let mut stream: surrealdb::Stream<Vec<surrealdb::types::Value>> = match stream_result {
        Ok(s) => {
            info!("Notification LIVE stream connected");
            s
        }
        Err(e) => {
            error!("Failed to start LIVE SELECT: {}", e);
            return Err(Box::new(e));
        }
    };

    while let Some(result) = stream.next().await {
        match result {
            Ok(notification) => {
                let action = format!("{:?}", notification.action).to_lowercase();

                // Log raw data to understand the format
                let data_debug = format!("{:?}", notification.data);
                info!(
                    "LIVE event: action={} data={}",
                    action,
                    &data_debug[..data_debug.len().min(200)]
                );

                if let Some(pid) = extract_person_id_from_debug(&data_debug) {
                    info!("Broadcasting to {}", pid);
                    let _ = tx.send(NotificationEvent {
                        person_id: pid,
                        action,
                    });
                } else {
                    warn!(
                        "Failed to extract person_id from: {}",
                        &data_debug[..data_debug.len().min(300)]
                    );
                }
            }
            Err(e) => {
                error!("LIVE stream recv error: {}", e);
                return Err(Box::new(e));
            }
        }
    }

    Ok(())
}

/// Extract person_id from the debug representation of the Value.
/// Handles format: RecordId(RecordId { table: Table("person"), key: String("abc123") })
fn extract_person_id_from_debug(debug_str: &str) -> Option<String> {
    // Find person_id field
    let pid_idx = debug_str.find("person_id")?;
    let rest = &debug_str[pid_idx..];

    // Look for Table("person") pattern
    let table_idx = rest.find("Table(\"person\")")?;
    let after_table = &rest[table_idx..];

    // Find key: String("xxxxx")
    let key_idx = after_table.find("key: String(\"")?;
    let key_start = key_idx + "key: String(\"".len();
    let key_rest = &after_table[key_start..];
    let key_end = key_rest.find('"')?;
    let key = &key_rest[..key_end];

    if !key.is_empty() {
        Some(format!("person:{}", key))
    } else {
        None
    }
}
