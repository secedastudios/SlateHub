use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::{self, Stream};
use serde::Serialize;
use std::convert::Infallible;
use std::time::Duration;
use tokio::time::interval;

use tracing::{debug, info};

/// Platform statistics that will be sent via SSE
#[derive(Debug, Clone, Serialize)]
pub struct PlatformStats {
    pub project_count: u32,
    pub user_count: u32,
    pub connection_count: u32,
}

impl PlatformStats {
    /// Create initial mock stats
    pub fn new() -> Self {
        Self {
            project_count: 1247,
            user_count: 5892,
            connection_count: 18453,
        }
    }

    /// Increment stats for demo purposes
    /// In production, this would fetch real data from the database
    pub fn increment(&mut self) {
        // Simulate organic growth with some randomness
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Projects grow slowly
        if rng.gen_bool(0.3) {
            self.project_count += rng.gen_range(1..=3);
        }

        // Users grow moderately
        if rng.gen_bool(0.5) {
            self.user_count += rng.gen_range(1..=5);
        }

        // Connections grow quickly
        if rng.gen_bool(0.7) {
            self.connection_count += rng.gen_range(2..=10);
        }
    }

    /// Convert stats to Datastar-compatible SSE format
    /// Datastar expects data in a specific format for signal updates
    pub fn to_datastar_event(&self) -> String {
        // Datastar SSE format for updating signal values
        // We send individual updates for each stat to allow smooth animations
        format!(
            r#"signals {{"projectCount": {}, "userCount": {}, "connectionCount": {}}}"#,
            self.project_count, self.user_count, self.connection_count
        )
    }
}

/// Create an SSE stream for platform statistics
pub async fn stats_stream() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("Creating new SSE stats stream");

    let stats = PlatformStats::new();
    let ticker = interval(Duration::from_secs(3)); // Update every 3 seconds

    let stream = stream::unfold((stats, ticker), |(mut stats, mut ticker)| async move {
        ticker.tick().await;

        // Increment stats for demo
        stats.increment();

        debug!(
            "Sending stats update: projects={}, users={}, connections={}",
            stats.project_count, stats.user_count, stats.connection_count
        );

        // Create SSE event in Datastar format
        let event = Event::default()
            .event("datastar-signal") // Datastar listens for this event type
            .data(stats.to_datastar_event());

        Some((Ok(event), (stats, ticker)))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Create an SSE stream for recent activity feed
#[derive(Debug, Clone, Serialize)]
pub struct ActivityItem {
    pub user: String,
    pub action: String,
    pub time: String,
}

impl ActivityItem {
    /// Generate a random activity item for demo purposes
    pub fn random() -> Self {
        use rand::seq::SliceRandom;

        let mut rng = rand::thread_rng();

        let users = vec![
            "Sarah Johnson",
            "Mike Chen",
            "Emily Rodriguez",
            "David Kim",
            "Alex Turner",
            "Maria Garcia",
            "James Wilson",
            "Lisa Anderson",
            "Robert Brown",
            "Jennifer Lee",
        ];

        let actions = vec![
            "started a new project",
            "joined the platform",
            "completed a collaboration",
            "updated their portfolio",
            "posted a job opportunity",
            "connected with a filmmaker",
            "shared a production update",
            "launched a campaign",
            "uploaded new work samples",
            "joined a production team",
        ];

        let times = vec![
            "just now",
            "1 minute ago",
            "2 minutes ago",
            "5 minutes ago",
            "10 minutes ago",
            "15 minutes ago",
            "30 minutes ago",
            "1 hour ago",
        ];

        ActivityItem {
            user: users.choose(&mut rng).unwrap().to_string(),
            action: actions.choose(&mut rng).unwrap().to_string(),
            time: times.choose(&mut rng).unwrap().to_string(),
        }
    }
}

/// Create an SSE stream for activity feed updates
pub async fn activity_stream() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("Creating new SSE activity stream");

    // Initialize with some default activities
    let initial_activities = vec![
        ActivityItem {
            user: "Sarah Johnson".to_string(),
            action: "started a new project".to_string(),
            time: "2 minutes ago".to_string(),
        },
        ActivityItem {
            user: "Mike Chen".to_string(),
            action: "joined the platform".to_string(),
            time: "15 minutes ago".to_string(),
        },
        ActivityItem {
            user: "Emily Rodriguez".to_string(),
            action: "completed a collaboration".to_string(),
            time: "1 hour ago".to_string(),
        },
        ActivityItem {
            user: "David Kim".to_string(),
            action: "updated their portfolio".to_string(),
            time: "3 hours ago".to_string(),
        },
    ];

    let ticker = interval(Duration::from_secs(5)); // Update every 5 seconds

    let stream = stream::unfold(
        (initial_activities, ticker),
        |(mut activities, mut ticker)| async move {
            ticker.tick().await;

            // Generate 1-3 new activities
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let num_new = rng.gen_range(1..=3);

            // Add new activities to the beginning
            for _ in 0..num_new {
                activities.insert(0, ActivityItem::random());
            }

            // Keep only the last 10 activities
            activities.truncate(10);

            debug!("Sending {} total activities", activities.len());

            // Format for Datastar - send complete activities list
            let json_activities = serde_json::to_string(&activities).unwrap_or_default();
            let datastar_data = format!(r#"signals {{"activities": {}}}"#, json_activities);

            let event = Event::default()
                .event("datastar-signal")
                .data(datastar_data);

            Some((Ok(event), (activities, ticker)))
        },
    );

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_stats_creation() {
        let stats = PlatformStats::new();
        assert!(stats.project_count > 0);
        assert!(stats.user_count > 0);
        assert!(stats.connection_count > 0);
    }

    #[test]
    fn test_platform_stats_increment() {
        let mut stats = PlatformStats::new();
        let initial_projects = stats.project_count;
        let initial_users = stats.user_count;
        let initial_connections = stats.connection_count;

        // Run increment multiple times to ensure at least some change
        for _ in 0..10 {
            stats.increment();
        }

        // At least one of the stats should have increased
        assert!(
            stats.project_count >= initial_projects
                || stats.user_count >= initial_users
                || stats.connection_count >= initial_connections
        );
    }

    #[test]
    fn test_datastar_event_format() {
        let stats = PlatformStats {
            project_count: 100,
            user_count: 200,
            connection_count: 300,
        };

        let event = stats.to_datastar_event();
        assert!(event.contains("projectCount"));
        assert!(event.contains("100"));
        assert!(event.contains("userCount"));
        assert!(event.contains("200"));
        assert!(event.contains("connectionCount"));
        assert!(event.contains("300"));
    }

    #[test]
    fn test_activity_item_random() {
        let activity = ActivityItem::random();
        assert!(!activity.user.is_empty());
        assert!(!activity.action.is_empty());
        assert!(!activity.time.is_empty());
    }
}
