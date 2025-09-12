-- ------------------------------
-- OPTION
-- ------------------------------

OPTION IMPORT;

-- ------------------------------
-- ENUM TABLES (for dynamic lists queryable by frontend)
-- These are simple tables holding possible values. Frontend can SELECT * FROM them.
-- We use SCHEMALESS for flexibility, but add a 'value' field.
-- ------------------------------

-- Organization Types
DEFINE TABLE organization_type SCHEMALESS PERMISSIONS FULL;
DEFINE FIELD value ON organization_type TYPE string;
DEFINE INDEX idx_organization_type_value ON organization_type FIELDS value UNIQUE;

-- Production Types
DEFINE TABLE production_type SCHEMALESS PERMISSIONS FULL;
DEFINE FIELD value ON production_type TYPE string;
DEFINE INDEX idx_production_type_value ON production_type FIELDS value UNIQUE;

-- Production Statuses
DEFINE TABLE production_status SCHEMALESS PERMISSIONS FULL;
DEFINE FIELD value ON production_status TYPE string;
DEFINE INDEX idx_production_status_value ON production_status FIELDS value UNIQUE;

-- Involvement Relation Types
DEFINE TABLE involvement_relation_type SCHEMALESS PERMISSIONS FULL;
DEFINE FIELD value ON involvement_relation_type TYPE string;
DEFINE INDEX idx_involvement_relation_type_value ON involvement_relation_type FIELDS value UNIQUE;

-- Roles
DEFINE TABLE role SCHEMALESS PERMISSIONS FULL;
DEFINE FIELD value ON role TYPE string;
DEFINE FIELD department ON role TYPE option<record<department>>;  -- Link to department if needed
DEFINE INDEX idx_role_value ON role FIELDS value UNIQUE;

-- Departments
DEFINE TABLE department SCHEMALESS PERMISSIONS FULL;
DEFINE FIELD value ON department TYPE string;
DEFINE INDEX idx_department_value ON department FIELDS value UNIQUE;

-- Phases
DEFINE TABLE phase SCHEMALESS PERMISSIONS FULL;
DEFINE FIELD value ON phase TYPE string;
DEFINE INDEX idx_phase_value ON phase FIELDS value UNIQUE;

-- Credit Types
DEFINE TABLE credit_type SCHEMALESS PERMISSIONS FULL;
DEFINE FIELD value ON credit_type TYPE string;
DEFINE INDEX idx_credit_type_value ON credit_type FIELDS value UNIQUE;

-- Ownership Types
DEFINE TABLE ownership_type SCHEMALESS PERMISSIONS FULL;
DEFINE FIELD value ON ownership_type TYPE string;
DEFINE INDEX idx_ownership_type_value ON ownership_type FIELDS value UNIQUE;

-- Additional Enum Tables for Profile Fields
-- Genders
DEFINE TABLE gender SCHEMALESS PERMISSIONS FULL;
DEFINE FIELD value ON gender TYPE string;
DEFINE INDEX idx_gender_value ON gender FIELDS value UNIQUE;

-- Ethnicities
DEFINE TABLE ethnicity SCHEMALESS PERMISSIONS FULL;
DEFINE FIELD value ON ethnicity TYPE string;
DEFINE INDEX idx_ethnicity_value ON ethnicity FIELDS value UNIQUE;

-- Hair Colors
DEFINE TABLE hair_color SCHEMALESS PERMISSIONS FULL;
DEFINE FIELD value ON hair_color TYPE string;
DEFINE INDEX idx_hair_color_value ON hair_color FIELDS value UNIQUE;

-- Eye Colors
DEFINE TABLE eye_color SCHEMALESS PERMISSIONS FULL;
DEFINE FIELD value ON eye_color TYPE string;
DEFINE INDEX idx_eye_color_value ON eye_color FIELDS value UNIQUE;

-- Builds/Body Types
DEFINE TABLE body_type SCHEMALESS PERMISSIONS FULL;
DEFINE FIELD value ON body_type TYPE string;
DEFINE INDEX idx_body_type_value ON body_type FIELDS value UNIQUE;

-- Unions
DEFINE TABLE union SCHEMALESS PERMISSIONS FULL;
DEFINE FIELD value ON union TYPE string;
DEFINE INDEX idx_union_value ON union FIELDS value UNIQUE;

-- ------------------------------
-- TABLE: media
-- ------------------------------

DEFINE TABLE media TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD caption ON media TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD is_public ON media TYPE bool DEFAULT false PERMISSIONS FULL;
DEFINE FIELD mime_type ON media TYPE string PERMISSIONS FULL;
DEFINE FIELD name ON media TYPE string PERMISSIONS FULL;
DEFINE FIELD uri ON media TYPE string PERMISSIONS FULL;

-- ------------------------------
-- TABLE: organization
-- ------------------------------

DEFINE TABLE organization TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD name ON organization TYPE string PERMISSIONS FULL;
DEFINE FIELD slug ON organization TYPE string VALUE string::slug($this.name) PERMISSIONS FULL;
DEFINE FIELD type ON organization TYPE string PERMISSIONS FULL;  -- From organization_type
DEFINE FIELD description ON organization TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD location ON organization TYPE option<string> PERMISSIONS FULL;  -- e.g., "Los Angeles, CA" for search
DEFINE FIELD website ON organization TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD social_links ON organization TYPE array<object> PERMISSIONS FULL;

DEFINE FIELD logo ON organization TYPE option<record<media>> PERMISSIONS FULL;
DEFINE FIELD contact_email ON organization TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD phone ON organization TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD services ON organization TYPE array<string> PERMISSIONS FULL;  -- e.g., ["vfx", "casting"]
DEFINE FIELD founded_year ON organization TYPE option<int> PERMISSIONS FULL;
DEFINE FIELD employees_count ON organization TYPE option<int> PERMISSIONS FULL;

-- ------------------------------
-- TABLE: owns
-- ------------------------------

DEFINE TABLE owns TYPE RELATION FROM person|organization TO organization|production SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD ownership_type ON owns TYPE option<string> PERMISSIONS FULL;  -- From ownership_type
DEFINE FIELD percentage ON owns TYPE option<float> PERMISSIONS FULL;
DEFINE FIELD start_date ON owns TYPE datetime PERMISSIONS FULL;
DEFINE FIELD end_date ON owns TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD timestamp ON owns TYPE datetime DEFAULT time::now() PERMISSIONS FULL;

-- ------------------------------
-- TABLE: person
-- ------------------------------

DEFINE TABLE person TYPE NORMAL SCHEMAFULL PERMISSIONS FULL;

DEFINE FIELD email ON person TYPE string ASSERT string::is::email($value) PERMISSIONS FULL;
DEFINE FIELD password ON person TYPE string PERMISSIONS FULL;
DEFINE FIELD profile ON person FLEXIBLE TYPE option<object> PERMISSIONS FULL;
DEFINE FIELD profile.avatar ON person TYPE option<string> PERMISSIONS FULL;  -- Profile image URL
DEFINE FIELD profile.headline ON person TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD profile.height_mm ON person TYPE option<int> PERMISSIONS FULL;
DEFINE FIELD profile.weight_kg ON person TYPE option<int> PERMISSIONS FULL;  -- Added for physical attributes
DEFINE FIELD profile.body_type ON person TYPE option<string> PERMISSIONS FULL;  -- From body_type enum
DEFINE FIELD profile.hair_color ON person TYPE option<string> PERMISSIONS FULL;  -- From hair_color
DEFINE FIELD profile.eye_color ON person TYPE option<string> PERMISSIONS FULL;  -- From eye_color
DEFINE FIELD profile.gender ON person TYPE option<string> PERMISSIONS FULL;  -- From gender
DEFINE FIELD profile.ethnicity ON person TYPE array<string> PERMISSIONS FULL;  -- From ethnicity
DEFINE FIELD profile.age_range ON person TYPE option<object> PERMISSIONS FULL;
DEFINE FIELD profile.age_range.min ON person TYPE int;
DEFINE FIELD profile.age_range.max ON person TYPE int;
DEFINE FIELD profile.media_other ON person TYPE array<record<media>> PERMISSIONS FULL;

DEFINE FIELD profile.reels ON person TYPE array<record<media>> PERMISSIONS FULL;  -- Demo reels/videos

DEFINE FIELD profile.name ON person TYPE string PERMISSIONS FULL;
DEFINE FIELD profile.skills ON person TYPE array<string> PERMISSIONS FULL;
DEFINE FIELD profile.bio ON person TYPE option<string> PERMISSIONS FULL;  -- For semantic search (detailed description)
DEFINE FIELD profile.social_links ON person TYPE array<object> PERMISSIONS FULL;

DEFINE FIELD profile.location ON person TYPE option<string> PERMISSIONS FULL;  -- e.g., city/state for search
DEFINE FIELD profile.unions ON person TYPE array<string> PERMISSIONS FULL;  -- From union enum, e.g., ["SAG-AFTRA", "IATSE"]
DEFINE FIELD profile.languages ON person TYPE array<string> PERMISSIONS FULL;  -- e.g., ["English", "Spanish"]
DEFINE FIELD profile.availability ON person TYPE option<string> PERMISSIONS FULL;  -- e.g., "full-time", "freelance", dates
DEFINE FIELD profile.experience ON person TYPE array<object> PERMISSIONS FULL;  -- For semantic (past roles/projects/credits)

DEFINE FIELD profile.education ON person TYPE array<object> PERMISSIONS FULL;  -- Training/schools

DEFINE FIELD profile.awards ON person TYPE array<object> PERMISSIONS FULL;

DEFINE FIELD profile.resume ON person TYPE option<record<media>> PERMISSIONS FULL;  -- PDF resume upload
DEFINE FIELD profile.website ON person TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD profile.phone ON person TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD username ON person TYPE string VALUE string::lowercase($value) PERMISSIONS FOR select, update FULL, FOR create WHERE FULL;

DEFINE INDEX person_username_unique ON person FIELDS username UNIQUE;
DEFINE INDEX person_email_unique ON person FIELDS email;
DEFINE INDEX idx_person_location ON person FIELDS profile.location;  -- For search
DEFINE INDEX idx_person_skills ON person FIELDS profile.skills;

-- ------------------------------
-- TABLE: production
-- ------------------------------

DEFINE TABLE production TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD title ON production TYPE string PERMISSIONS FULL;
DEFINE FIELD slug ON production TYPE string VALUE string::slug($this.title) PERMISSIONS FULL;
DEFINE FIELD type ON production TYPE string PERMISSIONS FULL;  -- From production_type
DEFINE FIELD status ON production TYPE string PERMISSIONS FULL;  -- From production_status
DEFINE FIELD start_date ON production TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD end_date ON production TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD description ON production TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD location ON production TYPE option<string> PERMISSIONS FULL;  -- For job-related search

-- ------------------------------
-- RELATION: part_of (for production hierarchy, e.g., episode part_of season, season part_of series)
-- ------------------------------

DEFINE TABLE part_of TYPE RELATION FROM production TO production SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD timestamp ON part_of TYPE datetime DEFAULT time::now() PERMISSIONS FULL;

-- ------------------------------
-- RELATION: involvement (generic for person/organization to production)
-- ------------------------------

DEFINE TABLE involvement TYPE RELATION FROM person|organization TO production SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD relation_type ON involvement TYPE string PERMISSIONS FULL;  -- From involvement_relation_type
DEFINE FIELD role ON involvement TYPE option<string> PERMISSIONS FULL;  -- From role
DEFINE FIELD department ON involvement TYPE option<string> PERMISSIONS FULL;  -- From department
DEFINE FIELD phase ON involvement TYPE option<string> PERMISSIONS FULL;  -- From phase
DEFINE FIELD credit_type ON involvement TYPE option<string> PERMISSIONS FULL;  -- From credit_type
DEFINE FIELD status ON involvement TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD dates ON involvement TYPE object PERMISSIONS FULL; -- Flexible {start: datetime, end: datetime, join_date: datetime}
DEFINE FIELD timestamp ON involvement TYPE datetime DEFAULT time::now() PERMISSIONS FULL;

-- ------------------------------
-- TABLE: job_posting (for jobs/casting calls)
-- ------------------------------

DEFINE TABLE job_posting TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD title ON job_posting TYPE string PERMISSIONS FULL;
DEFINE FIELD description ON job_posting TYPE string PERMISSIONS FULL;  -- Detailed text for semantic search
DEFINE FIELD requirements ON job_posting TYPE option<string> PERMISSIONS FULL;  -- Skills/qualifications text
DEFINE FIELD location ON job_posting TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD pay_rate ON job_posting TYPE option<string> PERMISSIONS FULL;  -- e.g., "hourly $50", "union scale"
DEFINE FIELD job_type ON job_posting TYPE string PERMISSIONS FULL;  -- e.g., "full_time", "freelance", "gig"
DEFINE FIELD start_date ON job_posting TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD end_date ON job_posting TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD posted_by ON job_posting TYPE record<person|organization> PERMISSIONS FULL;  -- Who posted
DEFINE FIELD related_production ON job_posting TYPE option<record<production>> PERMISSIONS FULL;
DEFINE FIELD status ON job_posting TYPE string DEFAULT "open" ASSERT $value INSIDE ["open", "closed", "filled"] PERMISSIONS FULL;
DEFINE FIELD created_at ON job_posting TYPE datetime DEFAULT time::now() PERMISSIONS FULL;

DEFINE INDEX idx_job_location ON job_posting FIELDS location;
DEFINE INDEX idx_job_type ON job_posting FIELDS job_type;

-- ------------------------------
-- RELATION: application (persons apply to jobs)
-- ------------------------------

DEFINE TABLE application TYPE RELATION FROM person TO job_posting SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD cover_letter ON application TYPE option<string> PERMISSIONS FULL;  -- Application text
DEFINE FIELD status ON application TYPE string DEFAULT "submitted" ASSERT $value INSIDE ["submitted", "reviewed", "interviewed", "hired", "rejected"] PERMISSIONS FULL;
DEFINE FIELD applied_at ON application TYPE datetime DEFAULT time::now() PERMISSIONS FULL;

-- ------------------------------
-- INDEXES (for performance, including semantic prep)
-- ------------------------------

DEFINE INDEX idx_production_type ON production FIELDS type;
DEFINE INDEX idx_organization_type ON organization FIELDS type;
DEFINE INDEX idx_involvement_role ON involvement FIELDS role;
DEFINE INDEX idx_involvement_relation_type ON involvement FIELDS relation_type;

-- Additional for search
DEFINE ANALYZER profile_analyzer TOKENIZERS blank,class FILTERS lowercase,snowball(english);
DEFINE INDEX idx_person_bio ON person FIELDS profile.bio SEARCH ANALYZER profile_analyzer BM25;
DEFINE INDEX idx_person_experience_desc ON person FIELDS profile.experience[*].description SEARCH ANALYZER profile_analyzer BM25;
DEFINE INDEX idx_job_description ON job_posting FIELDS description SEARCH ANALYZER profile_analyzer BM25;
DEFINE INDEX idx_job_requirements ON job_posting FIELDS requirements SEARCH ANALYZER profile_analyzer BM25;

-- ------------------------------
-- ACCESS DEFINITIONS (for authentication)
-- ------------------------------

-- User access for signup/signin
DEFINE ACCESS user ON DATABASE TYPE RECORD
    SIGNUP ( CREATE person SET username = string::lowercase($user), email = $email, password = crypto::argon2::generate($pass) )
    SIGNIN ( SELECT * FROM person WHERE (username = string::lowercase($user) OR email = $user) AND crypto::argon2::compare(password, $pass) )
    DURATION FOR TOKEN 15m, FOR SESSION 12h;

-- ------------------------------
-- INSERT STATEMENTS FOR ENUMS (Run these in a separate migration script or after schema creation)
-- ------------------------------

-- Organization Types
INSERT INTO organization_type (value) VALUES
("production_company"), ("film_studio"), ("tv_production_company"), ("commercial_production_company"),
("animation_studio"), ("vfx_studio"), ("talent_agency"), ("management_company"), ("distribution_company"),
("marketing_agency"), ("post_production_house"), ("casting_agency"), ("sound_studio"), ("catering_company"),
("equipment_rental"), ("location_services"), ("film_school"), ("community_group"), ("guild"), ("other");

-- Production Types
INSERT INTO production_type (value) VALUES
("film"), ("tv_series"), ("season"), ("episode"), ("documentary"), ("music_video"), ("commercial"),
("podcast"), ("webseries"), ("short_film"), ("animation"), ("reality_tv"), ("corporate_video"), ("other");

-- Production Statuses
INSERT INTO production_status (value) VALUES
("development"), ("pre_production"), ("production"), ("post_production"), ("completed"), ("released"), ("canceled");

-- Involvement Relation Types
INSERT INTO involvement_relation_type (value) VALUES
("owner"), ("admin"), ("member"), ("credit"), ("contractor"), ("other");

-- Departments
INSERT INTO department (value) VALUES
("above_the_line"), ("art"), ("camera"), ("sound"), ("lighting"), ("grip"), ("wardrobe"), ("makeup_hair"),
("production_management"), ("directing"), ("post_production"), ("casting"), ("stunts"), ("marketing"),
("distribution"), ("social_media"), ("vfx"), ("editing"), ("locations"), ("transportation"), ("other");

-- Roles (Comprehensive list from standard film crew positions)
INSERT INTO role (value, department) VALUES
("director", department:above_the_line), ("producer", department:above_the_line), ("executive_producer", department:above_the_line),
("writer", department:above_the_line), ("screenwriter", department:above_the_line), ("actor", department:cast),
("principal_cast", department:cast), ("production_designer", department:art), ("art_director", department:art),
("set_designer", department:art), ("set_decorator", department:art), ("prop_master", department:art),
("greensman", department:art), ("director_of_photography", department:camera), ("cinematographer", department:camera),
("camera_operator", department:camera), ("first_ac", department:camera), ("second_ac", department:camera),
("dit", department:camera), ("production_sound_mixer", department:sound), ("boom_operator", department:sound),
("sound_assistant", department:sound), ("gaffer", department:lighting), ("best_boy_electric", department:lighting),
("key_grip", department:grip), ("best_boy_grip", department:grip), ("costume_designer", department:wardrobe),
("costume_coordinator", department:wardrobe), ("tailor", department:wardrobe), ("shopper", department:wardrobe),
("makeup_artist", department:makeup_hair), ("key_makeup_artist", department:makeup_hair), ("hair_stylist", department:makeup_hair),
("key_hair_stylist", department:makeup_hair), ("line_producer", department:production_management),
("unit_production_manager", department:production_management), ("production_coordinator", department:production_management),
("production_assistant", department:production_management), ("location_manager", department:locations),
("location_scout", department:locations), ("first_ad", department:directing), ("second_ad", department:directing),
("script_supervisor", department:directing), ("editor", department:post_production), ("vfx_supervisor", department:vfx),
("colorist", department:post_production), ("sound_editor", department:post_production),
("casting_director", department:casting), ("stunt_coordinator", department:stunts), ("weapons_wrangler", department:stunts),
("on_set_editor", department:post_production), ("influencer", department:social_media), ("content_creator", department:social_media),
("brand_ambassador", department:marketing), ("social_media_manager", department:social_media), ("marketing_manager", department:marketing),
("distributor", department:distribution), ("agent", department:above_the_line), ("publicist", department:marketing),
("copywriter", department:marketing), ("video_editor", department:post_production), ("graphic_designer", department:art),
("photographer", department:camera), ("videographer", department:camera), ("driver", department:transportation),
("caterer", department:other), ("other", null);

-- Phases
INSERT INTO phase (value) VALUES
("development"), ("pre_production"), ("production"), ("post_production"), ("distribution"), ("all");

-- Credit Types
INSERT INTO credit_type (value) VALUES
("cast"), ("crew"), ("above_the_line"), ("below_the_line");

-- Ownership Types
INSERT INTO ownership_type (value) VALUES
("full"), ("partial"), ("subsidiary"), ("joint_venture"), ("other");

-- Genders
INSERT INTO gender (value) VALUES
("male"), ("female"), ("non_binary"), ("prefer_not_to_say"), ("other");

-- Ethnicities (representative list)
INSERT INTO ethnicity (value) VALUES
("african_american"), ("asian"), ("caucasian"), ("hispanic_latino"), ("native_american"), ("pacific_islander"),
("middle_eastern"), ("mixed"), ("other");

-- Hair Colors
INSERT INTO hair_color (value) VALUES
("black"), ("brown"), ("blonde"), ("red"), ("gray"), ("white"), ("bald"), ("other");

-- Eye Colors
INSERT INTO eye_color (value) VALUES
("brown"), ("blue"), ("green"), ("hazel"), ("gray"), ("black"), ("other");

-- Body Types
INSERT INTO body_type (value) VALUES
("athletic"), ("average"), ("slim"), ("curvy"), ("muscular"), ("petite"), ("tall"), ("other");

-- Unions
INSERT INTO union (value) VALUES
("sag_aftra"), ("iatse"), ("dga"), ("wga"), ("pga"), ("asc"), ("ace"), ("other");
