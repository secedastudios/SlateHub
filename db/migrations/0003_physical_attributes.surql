-- Migration 0003: Add physical attribute fields for casting search
-- New fields: birthday, acting_age_range, acting_ethnicities, nationality

DEFINE FIELD IF NOT EXISTS profile.birthday ON person TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD IF NOT EXISTS profile.acting_age_range ON person TYPE option<object> PERMISSIONS FULL;
DEFINE FIELD IF NOT EXISTS profile.acting_age_range.min ON person TYPE int;
DEFINE FIELD IF NOT EXISTS profile.acting_age_range.max ON person TYPE int;
DEFINE FIELD IF NOT EXISTS profile.acting_ethnicities ON person TYPE array<string> DEFAULT [] PERMISSIONS FULL;
DEFINE FIELD IF NOT EXISTS profile.nationality ON person TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD IF NOT EXISTS profile.is_public ON person TYPE bool DEFAULT false PERMISSIONS FULL;

-- Acting ethnicity lookup table for UI options
DEFINE TABLE IF NOT EXISTS acting_ethnicity TYPE NORMAL SCHEMAFULL PERMISSIONS FULL;
DEFINE FIELD IF NOT EXISTS name ON acting_ethnicity TYPE string;
DEFINE INDEX IF NOT EXISTS idx_acting_ethnicity_name ON acting_ethnicity FIELDS name UNIQUE;

INSERT INTO acting_ethnicity (name) VALUES
('African/Black'), ('Asian'), ('Caucasian/White'), ('East Asian'), ('Hispanic/Latino'),
('Indigenous'), ('Middle Eastern'), ('Mixed/Multiracial'), ('Pacific Islander'),
('South Asian'), ('Southeast Asian'), ('Other');

-- Update body_type seed with more industry-standard terms
INSERT IGNORE INTO body_type (name) VALUES ('Plus Size'), ('Slender');
