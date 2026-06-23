-- Migration 012: productions management — episodic structure, breakdown,
-- scheduling, and aristotle job tracking.
--
-- Squashed from pre-release migrations 012–016 (never deployed):
--   * seasons + episodes  (series support; `season` is REQUIRED on episode
--     because SurrealDB v3 UNIQUE indexes treat NULL as non-colliding —
--     flat series auto-create "Season 1" at the application layer)
--   * scene + breakdown_item  (aristotle parser output, owned by slatehub)
--   * scheduling  (schedule_day, schedule_scene, call_time, call_sheet)
--   * aristotle_job  (in-process breakdown run tracking)
--   * production_script.episode  (DEFINE FIELD OVERWRITE — retrofits the
--     field onto the pre-existing production_script table; fresh installs
--     already get it from schema.surql)
--
-- Statement order preserved from the original files: episodes before
-- scenes (scene references episode), scenes before scheduling and jobs.

--
-- Productions can be:
--   * One-off (feature, short, music video, vertical) — uses the existing
--     `production` model directly, no episodes or seasons.
--   * Series with one season (YouTube series, podcast feed, vertical drama
--     with one continuous arc) — a single un-titled `season` row with
--     season_number = 1; UI may hide the season grouping when only one
--     season with no title exists.
--   * Series with multiple seasons (traditional TV) — multiple `season`
--     rows, each grouping its own episodes.
--
-- **Every episode belongs to a season.** This avoids SurrealDB v3's
-- NULL-uniqueness semantics (NULLs don't collide on UNIQUE indexes —
-- Postgres-style). The application layer auto-creates "Season 1" when a
-- series gets its first episode and no season exists yet.
--
-- URL shape: `/productions/{prod_slug}/episodes/{ep_slug}` — season is
-- metadata, not URL structure. Promoting a single-season series to
-- multi-season is just adding more seasons + maybe titling Season 1.

DEFINE TABLE season TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD production ON season TYPE record<production> PERMISSIONS FULL;
DEFINE FIELD season_number ON season TYPE int ASSERT $value > 0 PERMISSIONS FULL;
DEFINE FIELD title ON season TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD description ON season TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD premiere_date ON season TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD finale_date ON season TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD status ON season TYPE string DEFAULT 'in_development'
    ASSERT $value IN ['in_development', 'in_production', 'released', 'on_hiatus', 'canceled'] PERMISSIONS FULL;
DEFINE FIELD created_at ON season TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD updated_at ON season TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_season_production_number ON season FIELDS production, season_number UNIQUE;
DEFINE INDEX idx_season_production ON season FIELDS production;

DEFINE TABLE episode TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD production ON episode TYPE record<production> PERMISSIONS FULL;
DEFINE FIELD season ON episode TYPE record<season> PERMISSIONS FULL;  -- always set; auto-created "Season 1" for flat series
DEFINE FIELD episode_number ON episode TYPE int ASSERT $value > 0 PERMISSIONS FULL;
DEFINE FIELD title ON episode TYPE string PERMISSIONS FULL;
DEFINE FIELD slug ON episode TYPE string ASSERT string::len($value) > 0 PERMISSIONS FULL;
DEFINE FIELD description ON episode TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD status ON episode TYPE string DEFAULT 'in_development'
    ASSERT $value IN ['in_development', 'in_production', 'shot', 'edited', 'released', 'canceled'] PERMISSIONS FULL;
DEFINE FIELD air_date ON episode TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD created_at ON episode TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD updated_at ON episode TYPE datetime VALUE time::now() PERMISSIONS FULL;

-- Uniqueness: (season, episode_number). Production scope is implied by
-- the season's own production. Two episodes in the same season can't
-- share a number; different seasons can each have their own episode 1.
DEFINE INDEX idx_episode_season_number ON episode FIELDS season, episode_number UNIQUE;

-- Per-production slug uniqueness so /productions/{prod}/episodes/{slug} resolves
DEFINE INDEX idx_episode_production_slug ON episode FIELDS production, slug UNIQUE;

DEFINE INDEX idx_episode_production ON episode FIELDS production;
DEFINE INDEX idx_episode_season ON episode FIELDS season;
--
-- A scene is a single shootable unit. It's produced by aristotle's parser
-- from a script and persisted here so slatehub owns the data, can render
-- it in the breakdown editor, and can attach it to a `schedule_scene` row
-- when planning a shoot day.
--
-- `episode` is nullable — production-level scripts (one-off films) just
-- have `episode = NULL`. Scene numbering is `(production, episode, scene_number)`.
--
-- A `breakdown_item` is one piece of metadata extracted from a scene:
-- a cast member, a prop, a wardrobe note, a VFX shot, etc. The `category`
-- ASSERT enumerates the categories slatehub renders chips for.

DEFINE TABLE scene TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD production ON scene TYPE record<production> PERMISSIONS FULL;
DEFINE FIELD episode ON scene TYPE option<record<episode>> PERMISSIONS FULL;
DEFINE FIELD script ON scene TYPE option<record<production_script>> PERMISSIONS FULL;  -- which script revision produced this scene
DEFINE FIELD scene_number ON scene TYPE string PERMISSIONS FULL;  -- "12" or "12A" — string for sub-numbers
DEFINE FIELD heading ON scene TYPE string PERMISSIONS FULL;       -- "INT. APARTMENT - DAY"
DEFINE FIELD int_ext ON scene TYPE option<string>
    ASSERT $value = NONE OR $value IN ['int', 'ext', 'int_ext'] PERMISSIONS FULL;
DEFINE FIELD location ON scene TYPE option<string> PERMISSIONS FULL;             -- text label; resolved to location record later
DEFINE FIELD location_record ON scene TYPE option<record<location>> PERMISSIONS FULL;
DEFINE FIELD time_of_day ON scene TYPE option<string> PERMISSIONS FULL;          -- "DAY", "NIGHT", "DAWN"
DEFINE FIELD page_length ON scene TYPE option<decimal> PERMISSIONS FULL;         -- e.g. 1.625
DEFINE FIELD description ON scene TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD status ON scene TYPE string DEFAULT 'auto'
    ASSERT $value IN ['auto', 'reviewed', 'shooting', 'wrapped'] PERMISSIONS FULL;
DEFINE FIELD created_at ON scene TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD updated_at ON scene TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_scene_production ON scene FIELDS production;
DEFINE INDEX idx_scene_episode ON scene FIELDS episode;
DEFINE INDEX idx_scene_script ON scene FIELDS script;

DEFINE TABLE breakdown_item TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD scene ON breakdown_item TYPE record<scene> PERMISSIONS FULL;
DEFINE FIELD category ON breakdown_item TYPE string
    ASSERT $value IN [
        'cast', 'cast_silent', 'cast_extra',
        'prop', 'wardrobe', 'makeup_hair',
        'sfx', 'vfx', 'sound_effect', 'stunt',
        'animal', 'vehicle', 'music',
        'location_note', 'introduction', 'other'
    ] PERMISSIONS FULL;
DEFINE FIELD value ON breakdown_item TYPE string PERMISSIONS FULL;       -- "Maya", "Briefcase", "Smoke Machine"
DEFINE FIELD confidence ON breakdown_item TYPE option<decimal> PERMISSIONS FULL;  -- 0.0–1.0 from aristotle
DEFINE FIELD source ON breakdown_item TYPE string DEFAULT 'auto'
    ASSERT $value IN ['auto', 'manual', 'edited'] PERMISSIONS FULL;       -- 'auto' = aristotle; 'manual' = user added; 'edited' = user modified an auto entry
DEFINE FIELD notes ON breakdown_item TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD created_at ON breakdown_item TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD updated_at ON breakdown_item TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_breakdown_item_scene ON breakdown_item FIELDS scene;
DEFINE INDEX idx_breakdown_item_scene_category ON breakdown_item FIELDS scene, category;
--
-- Hierarchy:
--   schedule_day        — one row per planned shoot date for a production/episode
--     ├── schedule_scene — many: scenes placed on this day with start times
--     ├── call_time      — many: per-person call times (one row per person per day)
--     └── call_sheet     — many: versioned call sheet snapshots for this day
--
-- A schedule_day belongs to a production (films, one-offs) or an episode
-- (series). Both `production` and `episode` are populated for series so
-- queries by production aggregate across all episodes; `episode` is NULL
-- for one-offs.

DEFINE TABLE schedule_day TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD production ON schedule_day TYPE record<production> PERMISSIONS FULL;
DEFINE FIELD episode ON schedule_day TYPE option<record<episode>> PERMISSIONS FULL;
DEFINE FIELD date ON schedule_day TYPE datetime PERMISSIONS FULL;
DEFINE FIELD location ON schedule_day TYPE option<record<location>> PERMISSIONS FULL;
DEFINE FIELD location_label ON schedule_day TYPE option<string> PERMISSIONS FULL;   -- free-text override / extra detail
DEFINE FIELD general_call ON schedule_day TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD breakfast_time ON schedule_day TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD lunch_time ON schedule_day TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD weather_summary ON schedule_day TYPE option<string> PERMISSIONS FULL;  -- snapshot at sheet-generation time
DEFINE FIELD sunrise ON schedule_day TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD sunset ON schedule_day TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD notes ON schedule_day TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD status ON schedule_day TYPE string DEFAULT 'planned'
    ASSERT $value IN ['planned', 'confirmed', 'shooting', 'wrapped', 'canceled'] PERMISSIONS FULL;
DEFINE FIELD created_at ON schedule_day TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD updated_at ON schedule_day TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_schedule_day_production_date ON schedule_day FIELDS production, date;
DEFINE INDEX idx_schedule_day_episode ON schedule_day FIELDS episode;

DEFINE TABLE schedule_scene TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD schedule_day ON schedule_scene TYPE record<schedule_day> PERMISSIONS FULL;
DEFINE FIELD scene ON schedule_scene TYPE record<scene> PERMISSIONS FULL;
DEFINE FIELD order_index ON schedule_scene TYPE int ASSERT $value >= 0 PERMISSIONS FULL;
DEFINE FIELD start_time ON schedule_scene TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD estimated_duration_min ON schedule_scene TYPE option<int> PERMISSIONS FULL;
DEFINE FIELD status ON schedule_scene TYPE string DEFAULT 'planned'
    ASSERT $value IN ['planned', 'shooting', 'wrapped', 'skipped'] PERMISSIONS FULL;
DEFINE FIELD created_at ON schedule_scene TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD updated_at ON schedule_scene TYPE datetime VALUE time::now() PERMISSIONS FULL;

-- A given scene appears at most once on a single shoot day
DEFINE INDEX idx_schedule_scene_day_scene ON schedule_scene FIELDS schedule_day, scene UNIQUE;
DEFINE INDEX idx_schedule_scene_day_order ON schedule_scene FIELDS schedule_day, order_index;

DEFINE TABLE call_time TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD schedule_day ON call_time TYPE record<schedule_day> PERMISSIONS FULL;
DEFINE FIELD person ON call_time TYPE record<person> PERMISSIONS FULL;
DEFINE FIELD role ON call_time TYPE option<string> PERMISSIONS FULL;       -- "Director", "1st AD", "Maya" (character) — display label
DEFINE FIELD department ON call_time TYPE option<string> PERMISSIONS FULL; -- from department enum, optional
DEFINE FIELD relation_type ON call_time TYPE option<string>
    ASSERT $value = NONE OR $value IN ['cast', 'crew'] PERMISSIONS FULL;
DEFINE FIELD call_at ON call_time TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD makeup_at ON call_time TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD on_set_at ON call_time TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD wrap_estimate ON call_time TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD work_status ON call_time TYPE option<string>
    ASSERT $value = NONE OR $value IN ['W', 'H', 'SW', 'WF', 'TR'] PERMISSIONS FULL;  -- Working / Hold / Start Work / Work Finish / Travel
DEFINE FIELD notes ON call_time TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD created_at ON call_time TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD updated_at ON call_time TYPE datetime VALUE time::now() PERMISSIONS FULL;

-- One row per (day, person) — a person has a single call time per shoot day
DEFINE INDEX idx_call_time_day_person ON call_time FIELDS schedule_day, person UNIQUE;
DEFINE INDEX idx_call_time_person ON call_time FIELDS person;

DEFINE TABLE call_sheet TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD schedule_day ON call_sheet TYPE record<schedule_day> PERMISSIONS FULL;
DEFINE FIELD version ON call_sheet TYPE int DEFAULT 1 ASSERT $value > 0 PERMISSIONS FULL;
DEFINE FIELD generated_at ON call_sheet TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD generated_by ON call_sheet TYPE option<record<person>> PERMISSIONS FULL;
DEFINE FIELD pdf_key ON call_sheet TYPE string PERMISSIONS FULL;            -- S3 key
DEFINE FIELD pdf_size ON call_sheet TYPE option<int> PERMISSIONS FULL;
DEFINE FIELD sent_at ON call_sheet TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD sent_by ON call_sheet TYPE option<record<person>> PERMISSIONS FULL;
DEFINE FIELD recipient_count ON call_sheet TYPE option<int> PERMISSIONS FULL;

DEFINE INDEX idx_call_sheet_day_version ON call_sheet FIELDS schedule_day, version UNIQUE;
DEFINE INDEX idx_call_sheet_day ON call_sheet FIELDS schedule_day;
--
-- Tracks in-process breakdown runs. The "aristotle" name refers to the
-- workspace crate (extracted from the standalone PoC) that does the
-- actual parsing + breakdown. Jobs run on tokio tasks inside slatehub —
-- no external service, no external job id, just our own tracking row.
--
-- Status transitions:
--   queued     → row created, task not yet picked up by the semaphore
--   running    → task started; started_at set
--   complete   → breakdown succeeded; scene + breakdown_item rows persisted; completed_at set
--   failed     → breakdown threw; error populated; completed_at set

DEFINE TABLE aristotle_job TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD production ON aristotle_job TYPE record<production> PERMISSIONS FULL;
DEFINE FIELD episode ON aristotle_job TYPE option<record<episode>> PERMISSIONS FULL;
DEFINE FIELD script ON aristotle_job TYPE record<production_script> PERMISSIONS FULL;
DEFINE FIELD status ON aristotle_job TYPE string DEFAULT 'queued'
    ASSERT $value IN ['queued', 'running', 'complete', 'failed'] PERMISSIONS FULL;
DEFINE FIELD scenes_produced ON aristotle_job TYPE option<int> PERMISSIONS FULL;
DEFINE FIELD error ON aristotle_job TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD created_at ON aristotle_job TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD started_at ON aristotle_job TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD completed_at ON aristotle_job TYPE option<datetime> PERMISSIONS FULL;

DEFINE INDEX idx_aristotle_job_production ON aristotle_job FIELDS production;
DEFINE INDEX idx_aristotle_job_script ON aristotle_job FIELDS script;
DEFINE INDEX idx_aristotle_job_status_created ON aristotle_job FIELDS status, created_at;
--
-- Production scripts can attach to either a production directly (films,
-- shorts, series bibles) or a specific episode (per-episode drafts).
-- The fresh-install schema.surql already declares this field; this
-- migration covers environments that applied 012/013/014/015 against a
-- production_script table created before the field existed.
--
-- DEFINE FIELD OVERWRITE makes this idempotent — re-running it on a DB
-- that already has the field is a no-op.

DEFINE FIELD OVERWRITE episode ON production_script TYPE option<record<episode>> PERMISSIONS FULL;

DEFINE INDEX IF NOT EXISTS idx_script_episode ON production_script FIELDS episode;
