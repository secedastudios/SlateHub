# Server-Sent Events (SSE) Implementation with Datastar

## Overview

This document describes the implementation of Server-Sent Events (SSE) in SlateHub, demonstrating real-time data updates using Axum, Tera templates, and Datastar.js for reactive UI updates.

## Architecture

### Backend Components

1. **SSE Module** (`server/src/sse.rs`)
   - Handles SSE stream generation
   - Provides mock data that increments periodically
   - Formats data for Datastar compatibility

2. **Routes** (`server/src/routes/mod.rs`)
   - `/api/sse/stats` - Platform statistics stream
   - `/api/sse/activity` - Activity feed stream

3. **Data Flow**
   ```
   Client (Datastar) → HTTP GET → Axum Route → SSE Stream → Periodic Updates → Client Store
   ```

## Implementation Details

### Server-Side (Rust/Axum)

#### Platform Statistics Stream

```rust
// Sends updates every 3 seconds
pub async fn stats_stream() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stats = PlatformStats::new();
    let ticker = interval(Duration::from_secs(3));
    
    let stream = stream::unfold((stats, ticker), |(mut stats, mut ticker)| async move {
        ticker.tick().await;
        stats.increment(); // Simulate growth
        
        let event = Event::default()
            .event("datastar-signal") // Datastar listens for this event type
            .data(stats.to_datastar_event());
        
        Some((Ok(event), (stats, ticker)))
    });
    
    Sse::new(stream).keep_alive(KeepAlive::default())
}
```

#### SSE Data Format

The SSE events are formatted specifically for Datastar:

```
event: datastar-signal
data: signals {"projectCount": 1250, "userCount": 5897, "connectionCount": 18463}
```

### Client-Side (HTML/Datastar)

#### Connecting to SSE

```html
<section 
    data-signals="{projectCount: 0, userCount: 0, connectionCount: 0}"
    data-on-load="@get('/api/sse/stats')"
>
    <!-- UI elements bound to store values -->
</section>
```

#### Reactive Updates

```html
<div class="stat-value" data-text="$projectCount.toLocaleString()">0</div>
```

The `$$get()` function in Datastar:
- Establishes an SSE connection
- Automatically parses incoming `datastar-store` events
- Updates the reactive store
- Triggers UI re-renders

## Features

### 1. Platform Statistics
- **Update Frequency**: Every 3 seconds
- **Data Points**: 
  - Project count
  - User count
  - Connection count
- **Growth Simulation**: Random increments to simulate organic growth

### 2. Activity Feed
- **Update Frequency**: Every 5 seconds
- **Data Format**: Array of activity items
- **Features**:
  - Prepends new activities
  - Maintains maximum of 10 items
  - Smooth animations on new items

## Testing

### Test Page
Access the test page at: `http://localhost:3000/static/test-sse.html`

This page provides:
- Visual confirmation of SSE connections
- Debug information
- Manual reconnection controls
- Real-time update monitoring

### Command-Line Testing

Use the provided test script:

```bash
./test_sse.sh
```

Or open the interactive test page at: `http://localhost:3000/static/test-sse.html`

Or test manually with curl:

```bash
# Test stats stream
curl -N -H "Accept: text/event-stream" http://localhost:3000/api/sse/stats

# Test activity stream
curl -N -H "Accept: text/event-stream" http://localhost:3000/api/sse/activity
```

## Integration with Templates

The main index page (`templates/index.html`) uses SSE for live updates:

```html
<!-- Platform Statistics Section -->
<section 
    class="stats-section"
    data-signals="{projectCount: 0, userCount: 0, connectionCount: 0}"
    data-on-load="@get('/api/sse/stats')"
>
    <!-- Stats display with automatic updates -->
</section>

<!-- Activity Feed Section -->
<div 
    class="activity-feed"
    data-signals="{activities: [], loading: false}"
    data-on-load="@get('/api/sse/activity')"
>
    <!-- Activity list with real-time updates -->
</div>
```

## Benefits

1. **Real-time Updates**: No polling required, server pushes updates
2. **Efficient**: Uses HTTP/1.1 standard, works through proxies
3. **Simple Integration**: Datastar handles all the complexity automatically - no need for custom JavaScript
4. **Graceful Degradation**: Falls back to static content if SSE fails
5. **Automatic Reconnection**: Built-in retry logic
6. **No Client-Side Logic**: Datastar automatically updates the signals when SSE events arrive

## Future Enhancements

### Database Integration
Replace mock data with real database queries:

```rust
// Future implementation
pub async fn stats_from_db() -> PlatformStats {
    let project_count = DB.query("SELECT count() FROM projects").await?;
    let user_count = DB.query("SELECT count() FROM users").await?;
    let connection_count = DB.query("SELECT count() FROM connections").await?;
    
    PlatformStats {
        project_count,
        user_count,
        connection_count,
    }
}
```

### Filtered Streams
Add support for user-specific or filtered streams:

```rust
// Stream only relevant activities for a user
pub async fn user_activity_stream(user_id: String) -> Sse<impl Stream> {
    // Filter activities based on user's network
}
```

### Performance Optimization
- Implement backpressure handling
- Add stream multiplexing for multiple clients
- Cache common data between streams

## Troubleshooting

### SSE Not Connecting
1. Check server is running: `curl http://localhost:3000/api/health`
2. Verify SSE endpoint: `curl -I http://localhost:3000/api/sse/stats`
3. Check browser console for errors
4. Ensure Datastar.js is loaded

### Updates Not Appearing
1. Verify event format matches `datastar-signal`
2. Check signal variable names match between server and client
3. Inspect network tab for SSE connection
4. Test with the standalone test page

### Performance Issues
1. Adjust update intervals in `sse.rs`
2. Limit number of concurrent connections
3. Implement connection pooling
4. Consider using WebSockets for high-frequency updates

## Dependencies

- **axum**: `0.8.4` - Web framework with SSE support
- **futures**: `0.3` - Stream utilities
- **tokio**: `1.47.1` - Async runtime
- **tokio-stream**: `0.1` - Stream extensions
- **rand**: `0.8` - Random number generation for mock data
- **serde**: `1.0` - Serialization
- **Datastar.js**: `0.19.10` - Client-side reactivity

## References

- [Axum SSE Documentation](https://docs.rs/axum/latest/axum/response/sse/index.html)
- [Datastar.js Documentation](https://datastar.fly.dev/)
- [Server-Sent Events Specification](https://html.spec.whatwg.org/multipage/server-sent-events.html)
- [MDN SSE Guide](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events)