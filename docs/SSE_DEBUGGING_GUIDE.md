# SSE Debugging Guide for SlateHub

## Issue Summary

The SSE (Server-Sent Events) implementation was not updating the UI correctly because of an incorrect event format being sent from the Rust backend to the Datastar frontend.

## The Fix

### Problem
1. **Wrong Event Type**: The backend was sending `datastar-signal` events instead of `datastar-patch-signals`
2. **Wrong Key Format**: The backend was using JSON format with quoted keys instead of JavaScript object literal notation with unquoted keys

### Solution
Changed the SSE event format in `server/src/sse.rs`:

**Before:**
```rust
Event::default()
    .event("datastar-signal")  // Wrong event type
    .data(format!(r#"signals {{"projectCount": {}, ...}}"#))  // Quoted keys (JSON)
```

**After:**
```rust
Event::default()
    .event("datastar-patch-signals")  // Correct event type
    .data(format!("signals {{projectCount: {}, ...}}"))  // Unquoted keys (JS object literal)
```

## Correct SSE Format for Datastar

### Event Structure
```
event: datastar-patch-signals
data: signals {key1: value1, key2: value2}
```

### Key Points
- Event type MUST be `datastar-patch-signals` (not `datastar-signal`)
- Data MUST start with `signals ` followed by JavaScript object literal notation
- Keys MUST NOT be quoted (JavaScript style, not JSON)
- Values can be any valid JavaScript expression

## Testing the SSE Implementation

### 1. Start the Server
```bash
cd server
cargo run
```

### 2. Test with Raw SSE Page
Open `http://localhost:3000/static/test-sse-raw.html` to see:
- Raw SSE events as they arrive
- Event types and data format
- Connection status

### 3. Test with Minimal Datastar Page
Open `http://localhost:3000/static/test-sse-minimal.html` to see:
- Clean Datastar implementation
- Live updating stats and activities
- Simpler UI for debugging

### 4. Test with Full Implementation
Open `http://localhost:3000/static/test-sse.html` to see:
- Full feature implementation
- Detailed debugging output in console

### 5. Command Line Testing
```bash
# Test stats endpoint
curl -N -H "Accept: text/event-stream" http://localhost:3000/api/sse/stats

# Test activity endpoint
curl -N -H "Accept: text/event-stream" http://localhost:3000/api/sse/activity

# Run format verification script
./test_sse_format.sh
```

## Common SSE Issues and Solutions

### Issue 1: UI Not Updating
**Symptoms:** SSE data arrives but UI doesn't update
**Check:**
- Event type is `datastar-patch-signals`
- Keys in signals object are unquoted
- Signal names match exactly between backend and frontend

### Issue 2: Connection Drops
**Symptoms:** SSE connection closes unexpectedly
**Check:**
- Server keeps connection alive with periodic keep-alive messages
- No timeout on server or proxy
- Check browser console for errors

### Issue 3: Data Format Errors
**Symptoms:** Console shows parsing errors
**Check:**
- No quotes around object keys
- Valid JavaScript expressions for values
- Proper escaping of special characters

## Browser Debugging

### Chrome DevTools
1. Open Network tab
2. Filter by "EventStream"
3. Click on SSE connection
4. View "EventStream" tab to see raw events

### Console Debugging
```javascript
// Override EventSource to log all events
const OriginalEventSource = window.EventSource;
window.EventSource = function(url) {
    console.log('SSE Connection:', url);
    const es = new OriginalEventSource(url);
    
    const originalAddEventListener = es.addEventListener;
    es.addEventListener = function(type, listener) {
        console.log('SSE Listener added for:', type);
        const wrappedListener = function(event) {
            console.log(`SSE Event [${type}]:`, event.data);
            listener.call(this, event);
        };
        return originalAddEventListener.call(this, type, wrappedListener);
    };
    
    return es;
};
```

## Datastar Signal Format Examples

### Simple Values
```javascript
signals {count: 42, message: "Hello", active: true}
```

### Arrays
```javascript
signals {items: ["item1", "item2", "item3"]}
```

### Complex Objects
```javascript
signals {activities: [{"user": "John", "action": "joined", "time": "now"}]}
```

## Backend Implementation Checklist

- [ ] Event type is `datastar-patch-signals`
- [ ] Data starts with `signals ` prefix
- [ ] Object keys are unquoted (JS style)
- [ ] Proper escaping of special characters
- [ ] Keep-alive messages are sent
- [ ] Content-Type header is `text/event-stream`
- [ ] Response is flushed after each event

## Frontend Implementation Checklist

- [ ] Datastar script is loaded
- [ ] `data-signals` defines initial state
- [ ] `data-on-load="@get('/api/sse/...')"` establishes connection
- [ ] Signal names match backend exactly
- [ ] Reactive bindings use `$signalName` syntax

## Useful Resources

- [Datastar SSE Events Documentation](https://data-star.dev/reference/sse_events)
- [MDN Server-Sent Events](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events)
- [SSE Specification](https://html.spec.whatwg.org/multipage/server-sent-events.html)