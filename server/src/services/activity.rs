use tracing::warn;

use crate::db::DB;

/// Fire-and-forget activity event. Spawns a background task so it never blocks requests.
pub fn log_activity(person_id: Option<&str>, event_type: &str, path: &str) {
    let person_id = person_id.map(|s| s.to_string());
    let event_type = event_type.to_string();
    let path = path.to_string();

    tokio::spawn(async move {
        let res = if let Some(pid) = &person_id {
            let pid_ref = if pid.starts_with("person:") {
                pid.clone()
            } else {
                format!("person:{}", pid)
            };
            let query = format!(
                "CREATE activity_event SET person_id = {}, event_type = $event_type, path = $path",
                pid_ref
            );
            DB.query(&query)
                .bind(("event_type", event_type))
                .bind(("path", path))
                .await
        } else {
            DB.query("CREATE activity_event SET event_type = $event_type, path = $path")
                .bind(("event_type", event_type))
                .bind(("path", path))
                .await
        };

        if let Err(e) = res {
            warn!(error = %e, "Failed to log activity event");
        }
    });
}
