use surrealdb::types::RecordId;
use tracing::{trace, warn};

use crate::db::DB;

/// Fire-and-forget activity event. Spawns a background task so it never blocks requests.
pub fn log_activity(person_id: Option<&str>, event_type: &str, path: &str) {
    let person_id = person_id.map(|s| s.to_string());
    let event_type = event_type.to_string();
    let path = path.to_string();

    tokio::spawn(async move {
        let res = if let Some(pid) = &person_id {
            // Parse "person:key" into a RecordId and bind it as a param
            // so SurrealDB always receives a proper record reference.
            let key = pid.strip_prefix("person:").unwrap_or(pid);
            let rid = RecordId::new("person", key);
            DB.query("CREATE activity_event SET person_id = $person_id, event_type = $event_type, path = $path")
                .bind(("person_id", rid))
                .bind(("event_type", event_type.clone()))
                .bind(("path", path.clone()))
                .await
        } else {
            DB.query("CREATE activity_event SET event_type = $event_type, path = $path")
                .bind(("event_type", event_type.clone()))
                .bind(("path", path.clone()))
                .await
        };

        match res {
            Ok(_) => trace!(event_type = %event_type, "Activity event logged"),
            Err(e) => warn!(error = %e, event_type = %event_type, "Failed to log activity event"),
        }
    });
}
