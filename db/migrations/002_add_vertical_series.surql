-- Migration 002: Add "Vertical Series" production type
-- Date: 2026-03-17

INSERT IGNORE INTO production_type (name) VALUES ("Vertical Series");
