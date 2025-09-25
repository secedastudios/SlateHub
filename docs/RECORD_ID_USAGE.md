# RecordId Usage Guidelines for SlateHub

## Overview

In SlateHub, all database entity IDs are stored as `surrealdb::RecordId` types. This document outlines the proper usage patterns for RecordId throughout the application, ensuring consistency and type safety.

## Key Principles

1. **Database IDs**: All struct fields representing database IDs MUST use `RecordId` type
2. **URL Generation**: Only use `record_id.key()` for URLs to get the clean ID without table prefix
3. **Method Parameters**: Database methods should accept `&RecordId` for ID parameters
4. **String Conversion**: Use `record_id.to_string()` for database queries and logging

## RecordId Structure

A `RecordId` in SurrealDB consists of two parts:
- **Table name**: The table the record belongs to (e.g., "production", "person", "organization")
- **Key**: The unique identifier within that table

Example: `production:abc123` where `production` is the table and `abc123` is the key.

## Usage Patterns

### 1. Model Struct Definition

```rust
use surrealdb::RecordId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Production {
    pub id: RecordId,  // NOT String
    pub title: String,
    pub slug: String,
    // ... other fields
}
```

### 2. Database Method Signatures

```rust
impl ProductionModel {
    // ✅ Correct - accepts &RecordId
    pub async fn get(production_id: &RecordId) -> Result<Production, Error> {
        // ...
    }
    
    // ✅ Correct - accepts &RecordId for the entity ID
    pub async fn update(
        production_id: &RecordId,
        data: UpdateProductionData,
    ) -> Result<Production, Error> {
        // ...
    }
    
    // ❌ Wrong - don't use &str for entity IDs
    pub async fn get(production_id: &str) -> Result<Production, Error> {
        // ...
    }
}
```

### 3. Database Queries

When binding RecordId to database queries, convert it to a string:

```rust
// ✅ Correct - convert RecordId to string for binding
let mut result = DB
    .query("SELECT * FROM $production_id")
    .bind(("production_id", production_id.to_string()))
    .await?;

// ✅ Also correct for DELETE, UPDATE, etc.
DB.query("DELETE FROM member_of WHERE out = $production_id")
    .bind(("production_id", production_id.to_string()))
    .await?;
```

### 4. URL Generation in Routes

When generating URLs or passing IDs to templates, use the `.key()` method:

```rust
// In route handlers
let template = ProductionTemplate {
    production: crate::templates::ProductionDetail {
        // ✅ Correct - use .key() for URLs
        id: production.id.key().to_string(),
        slug: production.slug,
        // ...
    }
};

// ❌ Wrong - don't try to strip prefixes manually
id: production.id.strip_prefix("production:").unwrap_or(&production.id).to_string(),
```

### 5. Logging and Display

Use `.to_string()` for logging:

```rust
debug!("Fetching production: {}", production_id);  // RecordId implements Display
info!("Updated production: {} ({})", production.title, production.id);
```

### 6. Foreign Keys and Relations

When dealing with related entities, store them as RecordId:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquipmentKit {
    pub id: RecordId,
    pub owner_person: Option<RecordId>,      // Foreign key to person table
    pub owner_organization: Option<RecordId>, // Foreign key to organization table
    // ...
}
```

### 7. Session User and Authentication

For session/auth contexts where you need string IDs, convert explicitly:

```rust
impl Person {
    pub fn to_session_user(&self) -> SessionUser {
        SessionUser {
            id: self.id.to_string(),  // Convert for session storage
            username: self.username.clone(),
            email: self.email.clone(),
            // ...
        }
    }
}
```

## Common Conversions

### RecordId → String (full ID with table prefix)
```rust
let full_id: String = record_id.to_string();
// Result: "production:abc123"
```

### RecordId → Key only (for URLs)
```rust
let key: String = record_id.key().to_string();
// Result: "abc123"
```

### String → RecordId (when needed)
```rust
use surrealdb::sql::Thing;

// If you have a full ID string
let thing = Thing::from(("production", "abc123"));
```

## Anti-Patterns to Avoid

1. **Don't use string manipulation for IDs**
   ```rust
   // ❌ Wrong
   production_id.strip_prefix("production:").unwrap_or(production_id)
   
   // ✅ Correct
   production_id.key()
   ```

2. **Don't store IDs as strings in models**
   ```rust
   // ❌ Wrong
   pub struct Production {
       pub id: String,
   }
   
   // ✅ Correct
   pub struct Production {
       pub id: RecordId,
   }
   ```

3. **Don't pass string IDs to database methods**
   ```rust
   // ❌ Wrong
   pub async fn get(id: &str) -> Result<Production, Error>
   
   // ✅ Correct
   pub async fn get(id: &RecordId) -> Result<Production, Error>
   ```

## Migration Checklist

When updating a model to use RecordId:

- [ ] Update the struct field from `String` to `RecordId`
- [ ] Update all method signatures to accept `&RecordId` instead of `&str`
- [ ] Replace string manipulations with `.key()` for URLs
- [ ] Update database query bindings to use `.to_string()`
- [ ] Update template data structures to use `.key().to_string()` for IDs
- [ ] Test that serialization/deserialization works correctly
- [ ] Verify foreign key references also use RecordId

## Examples from the Codebase

### Production Model
```rust
pub struct Production {
    pub id: RecordId,
    // ...
}

impl ProductionModel {
    pub async fn get(production_id: &RecordId) -> Result<Production, Error> {
        let mut result = DB
            .query("SELECT * FROM $production_id")
            .bind(("production_id", production_id.to_string()))
            .await?;
        // ...
    }
}
```

### Person Model
```rust
pub struct Person {
    pub id: RecordId,
    // ...
}

impl Person {
    pub async fn get(id: &RecordId) -> Result<Option<Self>> {
        match DB.select(id).await {
            Ok(person) => Ok(person),
            Err(e) => Err(e.into())
        }
    }
}
```

## Benefits of Using RecordId

1. **Type Safety**: Prevents mixing up IDs from different tables
2. **Consistency**: Single pattern for all database entities
3. **SurrealDB Integration**: Native support for SurrealDB's record system
4. **Clean URLs**: Easy extraction of just the key portion for URLs
5. **Future Proof**: Easier to change ID strategies if needed

## Summary

Always use `RecordId` for database entity IDs, use `.key()` for URLs, and `.to_string()` for database queries. This ensures type safety and consistency throughout the SlateHub application.