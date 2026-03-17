-- ============================================================================
-- SlateHub Mock Data Seed
-- Run: make db-seed-mock (or manually via surreal sql)
-- Prereq: make db-init && make db-seed (creates kevin, chris, Seceda)
-- ============================================================================

-- ============================================================================
-- 1. PEOPLE (20+ diverse profiles — various completeness levels)
-- ============================================================================

CREATE person SET
  username = 'sarahchen', email = 'sarah@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Sarah Chen', verification_status = 'identity',
  profile = {
    name: 'Sarah Chen', headline: 'Cinematographer / DP',
    bio: 'Award-winning cinematographer with 15 years of experience in narrative features and documentaries. Known for naturalistic lighting and handheld work. Shot two Sundance selections.',
    location: 'Los Angeles, CA', website: 'https://sarahchen.dp',
    is_public: true, gender: 'Female', ethnicity: ['Asian'],
    skills: ['Cinematography', 'Lighting Design', 'ARRI Alexa', 'RED Camera', 'Color Grading', 'Steadicam'],
    unions: ['IATSE', 'ASC'], languages: ['English', 'Mandarin'],
    availability: 'Available from April 2026',
    education: [], awards: [],
    social_links: [{ platform: 'instagram', url: 'https://instagram.com/sarahchenDP' }],
    reels: [{ url: 'https://vimeo.com/sarahchen/reel', title: 'Cinematography Reel 2025', platform: 'vimeo', video_id: 'sarahchen-reel' }],
    media_other: [], photos: []
  };

CREATE person SET
  username = 'marcusjohnson', email = 'marcus@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Marcus Johnson', verification_status = 'email',
  profile = {
    name: 'Marcus Johnson', headline: 'Writer, Director, Producer',
    bio: 'Independent filmmaker focused on stories about the African American experience in the modern South. My debut feature "Red Clay" premiered at SXSW 2024.',
    location: 'Atlanta, GA', is_public: true, gender: 'Male',
    ethnicity: ['African American'],
    skills: ['Directing', 'Screenwriting', 'Producing', 'Editing', 'Story Development'],
    unions: ['DGA', 'WGA'], languages: ['English'],
    education: [], awards: [],
    social_links: [], reels: [], media_other: [], photos: []
  };

CREATE person SET
  username = 'emiliarossi', email = 'emilia@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Emilia Rossi', verification_status = 'identity',
  profile = {
    name: 'Emilia Rossi', headline: 'Actor',
    bio: 'Classically trained actor. Theatre background with recent transition to screen. Fluent in Italian, English, and French. Comfortable with stage combat and dialect work.',
    location: 'London, UK', is_public: true, gender: 'Female',
    ethnicity: ['Caucasian'], hair_color: 'Brown', eye_color: 'Green',
    height_mm: 1680, body_type: 'Athletic',
    acting_ethnicities: ['Caucasian/White', 'Hispanic/Latino'],
    skills: ['Acting', 'Stage Combat', 'Dialect Coach', 'Improvisation', 'Meisner Technique'],
    unions: ['SAG-AFTRA'], languages: ['English', 'Italian', 'French'],
    availability: 'Available immediately',
    education: [],
    awards: [], social_links: [{ platform: 'imdb', url: 'https://imdb.com/name/nm000000' }],
    reels: [{ url: 'https://vimeo.com/emiliarossi/acting', title: 'Acting Showreel', platform: 'vimeo', video_id: 'emiliarossi-acting' }],
    media_other: [], photos: []
  };

CREATE person SET
  username = 'takeshiyamada', email = 'takeshi@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Takeshi Yamada', verification_status = 'email',
  profile = {
    name: 'Takeshi Yamada', headline: 'Sound Designer / Mixer',
    bio: 'Production and post-production sound for film and television. Specialize in immersive audio and Atmos mixes.',
    location: 'Tokyo, Japan', is_public: true, gender: 'Male',
    ethnicity: ['Asian'],
    skills: ['Sound Design', 'Sound Mixing', 'Dolby Atmos', 'Pro Tools', 'Field Recording', 'Foley'],
    unions: [], languages: ['Japanese', 'English'],
    education: [], awards: [],
    social_links: [], reels: [], media_other: [], photos: []
  };

CREATE person SET
  username = 'priyapatel', email = 'priya@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Priya Patel', verification_status = 'identity',
  profile = {
    name: 'Priya Patel', headline: 'Production Designer',
    bio: 'Creating immersive worlds for film and TV. From period pieces to sci-fi. Recently designed the Netflix limited series "The Partition" — nominated for an Emmy.',
    location: 'Mumbai, India', website: 'https://priyapateldesign.com',
    is_public: true, gender: 'Female', ethnicity: ['Asian'],
    skills: ['Production Design', 'Art Direction', 'Set Decoration', 'Concept Art', 'SketchUp', 'AutoCAD'],
    unions: ['IATSE'], languages: ['English', 'Hindi', 'Gujarati'],
    education: [], awards: [],
    social_links: [], reels: [], media_other: [], photos: []
  };

CREATE person SET
  username = 'alexmartinez', email = 'alex@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Alex Martinez', verification_status = 'unverified',
  profile = {
    name: 'Alex Martinez', headline: 'Editor',
    bio: 'Freelance editor. Features, docs, and branded content. Avid and Premiere.',
    location: 'Mexico City, Mexico', is_public: true, gender: 'Non-Binary',
    skills: ['Editing', 'Avid Media Composer', 'Adobe Premiere', 'DaVinci Resolve', 'Color Grading'],
    languages: ['Spanish', 'English'],
    social_links: [], reels: [], media_other: [], photos: [],
    ethnicity: [], unions: [], education: [], awards: []
  };

CREATE person SET
  username = 'lisanderson', email = 'lisa@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Lisa Anderson', verification_status = 'email',
  profile = {
    name: 'Lisa Anderson', headline: 'Costume Designer',
    bio: 'Period and contemporary costume design for film, TV, and theater. Skilled in pattern making, dyeing, and breakdown techniques.',
    location: 'New York, NY', is_public: true, gender: 'Female',
    skills: ['Costume Design', 'Pattern Making', 'Textile Dyeing', 'Period Research', 'Wardrobe Supervision'],
    unions: ['IATSE'], languages: ['English'],
    ethnicity: ['Caucasian'],
    education: [],
    awards: [],
    social_links: [], reels: [], media_other: [], photos: []
  };

CREATE person SET
  username = 'danielokafor', email = 'daniel@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Daniel Okafor', verification_status = 'identity',
  profile = {
    name: 'Daniel Okafor', headline: 'VFX Supervisor',
    bio: 'Senior VFX supervisor with credits on Marvel, A24, and BBC productions. Specialize in photoreal CG environments and creature work.',
    location: 'Vancouver, BC', is_public: true, gender: 'Male',
    ethnicity: ['African American'],
    skills: ['VFX Supervision', 'Nuke', 'Houdini', 'Maya', 'CG Environments', 'Compositing'],
    unions: ['IATSE'], languages: ['English', 'Igbo'],
    education: [], awards: [],
    social_links: [], reels: [], media_other: [], photos: []
  };

CREATE person SET
  username = 'hannamuller', email = 'hanna@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Hanna Müller', verification_status = 'email',
  profile = {
    name: 'Hanna Müller', headline: 'Producer',
    bio: 'Berlin-based producer. Focused on European co-productions and arthouse cinema. Recent films screened at Venice, Berlinale, and Cannes.',
    location: 'Berlin, Germany', is_public: true, gender: 'Female',
    ethnicity: ['Caucasian'],
    skills: ['Producing', 'Co-Production', 'Film Financing', 'Distribution', 'Sales'],
    unions: [], languages: ['German', 'English', 'French'],
    education: [],
    awards: [],
    social_links: [], reels: [], media_other: [], photos: []
  };

CREATE person SET
  username = 'jameswilson', email = 'james@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'James Wilson', verification_status = 'unverified',
  profile = {
    name: 'James Wilson', headline: 'Gaffer',
    bio: 'Feature film gaffer with 20 years experience. Union member.',
    location: 'Chicago, IL', is_public: true, gender: 'Male',
    skills: ['Lighting', 'Electrical', 'Rigging', 'DMX Programming'],
    unions: ['IATSE'], languages: ['English'],
    ethnicity: [], education: [], awards: [],
    social_links: [], reels: [], media_other: [], photos: []
  };

CREATE person SET
  username = 'minglee', email = 'ming@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Ming Lee', verification_status = 'email',
  profile = {
    name: 'Ming Lee', headline: 'Stunt Coordinator',
    bio: 'Martial arts and wire-work specialist. 12 years coordinating action sequences for Hong Kong and Hollywood productions.',
    location: 'Hong Kong', is_public: true, gender: 'Male',
    ethnicity: ['Asian'], body_type: 'Athletic',
    skills: ['Stunt Coordination', 'Wire Work', 'Martial Arts', 'Fight Choreography', 'Rigging'],
    languages: ['Cantonese', 'Mandarin', 'English'],
    ethnicity: [], unions: [], education: [], awards: [],
    social_links: [], reels: [], media_other: [], photos: []
  };

CREATE person SET
  username = 'sofiamorales', email = 'sofia@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Sofia Morales', verification_status = 'identity',
  profile = {
    name: 'Sofia Morales', headline: 'Casting Director',
    bio: 'Casting for independent and studio features. Known for discovering fresh talent. CSA member.',
    location: 'Los Angeles, CA', is_public: true, gender: 'Female',
    ethnicity: ['Hispanic/Latino'],
    skills: ['Casting', 'Talent Scouting', 'Audition Direction', 'Negotiation'],
    languages: ['English', 'Spanish'],
    unions: [], education: [], awards: [],
    social_links: [], reels: [], media_other: [], photos: []
  };

CREATE person SET
  username = 'ottovonberg', email = 'otto@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Otto von Berg', verification_status = 'email',
  profile = {
    name: 'Otto von Berg', headline: 'Composer / Music Supervisor',
    bio: 'Orchestral and electronic film scores. Recent work includes the Berlinale-selected "Winter Crossing" and several high-profile ad campaigns.',
    location: 'Vienna, Austria', is_public: true, gender: 'Male',
    skills: ['Film Scoring', 'Orchestration', 'Music Supervision', 'Logic Pro', 'Ableton'],
    languages: ['German', 'English'],
    ethnicity: [], unions: [], education: [], awards: [],
    social_links: [], reels: [], media_other: [], photos: []
  };

-- Minimal profile (edge case: barely filled out)
CREATE person SET
  username = 'ghostuser', email = 'ghost@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Ghost User', verification_status = 'unverified',
  profile = { is_public: true, social_links: [], reels: [], media_other: [], photos: [],
    ethnicity: [], unions: [], languages: [], education: [], awards: [], skills: [] };

-- Private profile (edge case: not public)
CREATE person SET
  username = 'privateperson', email = 'private@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Private Person', verification_status = 'email',
  profile = { name: 'Private Person', headline: 'Director', is_public: false,
    social_links: [], reels: [], media_other: [], photos: [],
    ethnicity: [], unions: [], languages: [], education: [], awards: [], skills: [] };

-- Long bio (edge case: very long content)
CREATE person SET
  username = 'verbosewriter', email = 'verbose@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Verbose McWritesalot', verification_status = 'email',
  profile = {
    name: 'Verbose McWritesalot', headline: 'Screenwriter / Story Consultant / Creative Director / Narrative Designer',
    bio: 'Twenty-five years of storytelling across every medium imaginable. I have written feature screenplays, television pilots, limited series, interactive narratives for games, podcast scripts, stage plays, and even a graphic novel. My work has taken me from Hollywood writers rooms to independent theatre in Edinburgh to game studios in Stockholm. I believe in the power of character-driven narratives and am passionate about mentoring the next generation of writers. Currently developing a limited series about climate migration and an animated feature about deep-sea exploration. Always open to collaborations that challenge conventions and push the boundaries of what stories can do.',
    location: 'Portland, OR', is_public: true,
    skills: ['Screenwriting', 'Story Development', 'TV Writing', 'Showrunning', 'Script Doctoring', 'Interactive Narrative', 'Podcast Writing', 'Playwriting', 'Graphic Novels', 'Mentoring'],
    languages: ['English', 'Swedish'],
    ethnicity: [], unions: ['WGA'], education: [], awards: [],
    social_links: [], reels: [], media_other: [], photos: []
  };

CREATE person SET
  username = 'rikaandersson', email = 'rika@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Rika Andersson', verification_status = 'identity',
  profile = {
    name: 'Rika Andersson', headline: 'Line Producer / UPM',
    bio: 'Experienced line producer managing budgets from $500K to $30M. Skilled in scheduling, union negotiations, and international co-productions.',
    location: 'Stockholm, Sweden', is_public: true, gender: 'Female',
    skills: ['Line Producing', 'Budgeting', 'Scheduling', 'Movie Magic', 'Union Negotiations', 'Co-Production'],
    languages: ['Swedish', 'English', 'Norwegian'],
    ethnicity: [], unions: ['PGA'], education: [], awards: [],
    social_links: [], reels: [], media_other: [], photos: []
  };

CREATE person SET
  username = 'jackflynn', email = 'jack@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Jack Flynn', verification_status = 'email',
  profile = {
    name: 'Jack Flynn', headline: 'Actor / Stunt Performer',
    bio: 'Action actor and stunt performer. Trained in parkour, kickboxing, and precision driving.',
    location: 'Dublin, Ireland', is_public: true, gender: 'Male',
    ethnicity: ['Caucasian'], hair_color: 'Red', eye_color: 'Green',
    height_mm: 1830, body_type: 'Athletic', acting_ethnicities: ['Caucasian/White'],
    skills: ['Acting', 'Stunt Performance', 'Parkour', 'Kickboxing', 'Precision Driving', 'Sword Fighting'],
    languages: ['English', 'Irish'],
    unions: ['SAG-AFTRA'], education: [], awards: [],
    social_links: [], reels: [], media_other: [], photos: []
  };

CREATE person SET
  username = 'naomitanaka', email = 'naomi@example.com',
  password = crypto::argon2::generate('pass123'),
  name = 'Naomi Tanaka', verification_status = 'email',
  profile = {
    name: 'Naomi Tanaka', headline: 'Makeup & Hair Department Head',
    bio: 'SFX makeup and period hair specialist. Credits include HBO, Amazon, and several feature horror films.',
    location: 'Los Angeles, CA', is_public: true, gender: 'Female',
    skills: ['SFX Makeup', 'Prosthetics', 'Period Hair', 'Wig Making', 'Aging Techniques'],
    languages: ['English', 'Japanese'],
    ethnicity: ['Asian'], unions: ['IATSE'], education: [], awards: [],
    social_links: [], reels: [], media_other: [], photos: []
  };

-- ============================================================================
-- 2. ORGANIZATIONS (10 diverse orgs)
-- ============================================================================

LET $sarah = (SELECT id FROM person WHERE username = 'sarahchen')[0].id;
LET $marcus = (SELECT id FROM person WHERE username = 'marcusjohnson')[0].id;
LET $hanna = (SELECT id FROM person WHERE username = 'hannamuller')[0].id;
LET $priya = (SELECT id FROM person WHERE username = 'priyapatel')[0].id;
LET $daniel = (SELECT id FROM person WHERE username = 'danielokafor')[0].id;
LET $sofia = (SELECT id FROM person WHERE username = 'sofiamorales')[0].id;
LET $rika = (SELECT id FROM person WHERE username = 'rikaandersson')[0].id;
LET $chris = (SELECT id FROM person WHERE username = 'chris')[0].id;
LET $kevin = (SELECT id FROM person WHERE username = 'kevin')[0].id;

LET $type_prod = (SELECT id FROM organization_type WHERE name = 'Production Company')[0].id;
LET $type_studio = (SELECT id FROM organization_type WHERE name = 'Film Studio')[0].id;
LET $type_vfx = (SELECT id FROM organization_type WHERE name = 'VFX Studio')[0].id;
LET $type_talent = (SELECT id FROM organization_type WHERE name = 'Talent Agency')[0].id;
LET $type_post = (SELECT id FROM organization_type WHERE name = 'Post Production House')[0].id;
LET $type_casting = (SELECT id FROM organization_type WHERE name = 'Casting Agency')[0].id;
LET $type_catering = (SELECT id FROM organization_type WHERE name = 'Catering Company')[0].id;
LET $type_dist = (SELECT id FROM organization_type WHERE name = 'Distribution Company')[0].id;
LET $type_anim = (SELECT id FROM organization_type WHERE name = 'Animation Studio')[0].id;
LET $type_equip = (SELECT id FROM organization_type WHERE name = 'Equipment Rental')[0].id;

CREATE organization SET name = 'Ember Films', slug = 'ember-films', type = $type_prod,
  description = 'Independent production company specializing in character-driven narratives. Based in Atlanta with a focus on Southern stories.',
  location = 'Atlanta, GA', public = true, verified = true,
  services = ['Feature Film Production', 'Documentary', 'Development'], social_links = [],
  founded_year = 2019, employees_count = 8;

CREATE organization SET name = 'Lichtwerk Studio', slug = 'lichtwerk', type = $type_studio,
  description = 'Full-service film studio in Berlin. Three stages, standing sets, and production offices available for rental.',
  location = 'Berlin, Germany', website = 'https://lichtwerk.studio', public = true, verified = true,
  services = ['Studio Rental', 'Standing Sets', 'Production Offices', 'Equipment'], social_links = [],
  founded_year = 2015, employees_count = 22;

CREATE organization SET name = 'Parallax VFX', slug = 'parallax-vfx', type = $type_vfx,
  description = 'Boutique VFX studio. Photoreal environments, creature work, and seamless compositing for film and premium TV.',
  location = 'Vancouver, BC', public = true, verified = false,
  services = ['CG Environments', 'Creature FX', 'Compositing', 'Previz'], social_links = [],
  founded_year = 2017, employees_count = 45;

CREATE organization SET name = 'Rosewood Talent', slug = 'rosewood-talent', type = $type_talent,
  description = 'Boutique talent agency representing actors, directors, and writers across film, TV, and theater.',
  location = 'Los Angeles, CA', public = true, verified = true,
  services = ['Actor Representation', 'Director Representation', 'Writer Representation'], social_links = [],
  founded_year = 2008, employees_count = 12;

CREATE organization SET name = 'Final Frame Post', slug = 'final-frame', type = $type_post,
  description = 'Color grading, online editing, and deliverables. DI suite with Dolby Vision and HDR capability.',
  location = 'London, UK', public = true, verified = false,
  services = ['Color Grading', 'Online Editing', 'DCP Mastering', 'HDR Finishing'], social_links = [],
  founded_year = 2020, employees_count = 6;

CREATE organization SET name = 'Morales Casting', slug = 'morales-casting', type = $type_casting,
  description = 'Casting for indie features, studio films, and streaming series. Passionate about inclusive casting.',
  location = 'Los Angeles, CA', public = true, verified = true,
  services = ['Feature Film Casting', 'TV Series Casting', 'Commercial Casting'], social_links = [],
  founded_year = 2016, employees_count = 5;

CREATE organization SET name = 'Nordic Light Films', slug = 'nordic-light', type = $type_prod,
  description = 'Scandinavian production company focused on international co-productions and genre cinema.',
  location = 'Stockholm, Sweden', public = true, verified = true,
  services = ['Feature Production', 'Co-Production', 'Development', 'Sales'], social_links = [],
  founded_year = 2012, employees_count = 10;

CREATE organization SET name = 'Craft Catering', slug = 'craft-catering', type = $type_catering,
  description = 'On-set catering for film and TV productions. Dietary accommodations for all crew sizes.',
  location = 'Los Angeles, CA', public = true, verified = false,
  services = ['On-Set Catering', 'Craft Services', 'Dietary Accommodations'], social_links = [],
  founded_year = 2021, employees_count = 15;

CREATE organization SET name = 'Keyframe Animation', slug = 'keyframe-anim', type = $type_anim,
  description = '2D and 3D animation studio. Feature films, series, and short-form content.',
  location = 'Tokyo, Japan', public = true, verified = false,
  services = ['2D Animation', '3D Animation', 'Character Design', 'Storyboarding'], social_links = [],
  founded_year = 2018, employees_count = 30;

-- Private org (edge case)
CREATE organization SET name = 'Stealth Productions', slug = 'stealth-productions', type = $type_prod,
  description = 'In-development production house. Not yet public.',
  location = 'New York, NY', public = false, verified = false,
  services = [], social_links = [];

-- ============================================================================
-- 3. ORG MEMBERSHIPS
-- ============================================================================

LET $ember = (SELECT id FROM organization WHERE slug = 'ember-films')[0].id;
LET $lichtwerk = (SELECT id FROM organization WHERE slug = 'lichtwerk')[0].id;
LET $parallax = (SELECT id FROM organization WHERE slug = 'parallax-vfx')[0].id;
LET $rosewood = (SELECT id FROM organization WHERE slug = 'rosewood-talent')[0].id;
LET $finalframe = (SELECT id FROM organization WHERE slug = 'final-frame')[0].id;
LET $moralescast = (SELECT id FROM organization WHERE slug = 'morales-casting')[0].id;
LET $nordic = (SELECT id FROM organization WHERE slug = 'nordic-light')[0].id;
LET $keyframe = (SELECT id FROM organization WHERE slug = 'keyframe-anim')[0].id;
LET $seceda = (SELECT id FROM organization WHERE slug = 'seceda')[0].id;

RELATE $marcus->member_of->$ember SET role = 'owner', invitation_status = 'accepted';
RELATE $hanna->member_of->$lichtwerk SET role = 'owner', invitation_status = 'accepted';
RELATE $daniel->member_of->$parallax SET role = 'owner', invitation_status = 'accepted';
RELATE $sofia->member_of->$rosewood SET role = 'admin', invitation_status = 'accepted';
RELATE $sarah->member_of->$finalframe SET role = 'owner', invitation_status = 'accepted';
RELATE $sofia->member_of->$moralescast SET role = 'owner', invitation_status = 'accepted';
RELATE $rika->member_of->$nordic SET role = 'owner', invitation_status = 'accepted';
RELATE $hanna->member_of->$nordic SET role = 'admin', invitation_status = 'accepted';
RELATE $kevin->member_of->$keyframe SET role = 'owner', invitation_status = 'accepted';

-- Cross-memberships
RELATE $sarah->member_of->$ember SET role = 'member', invitation_status = 'accepted';
RELATE $chris->member_of->$lichtwerk SET role = 'member', invitation_status = 'accepted';
RELATE $marcus->member_of->$nordic SET role = 'member', invitation_status = 'pending';

-- ============================================================================
-- 4. PRODUCTIONS (12 productions at various stages)
-- ============================================================================

CREATE production SET title = 'Red Clay', slug = 'red-clay', type = 'Film', status = 'Completed',
  description = 'A young Black woman returns to rural Georgia to settle her late grandmothers estate and uncovers long-buried family secrets that challenge everything she believed about her past.',
  location = 'Atlanta, GA', start_date = d'2023-03-15T00:00:00Z', end_date = d'2023-08-20T00:00:00Z',
  source = 'manual', source_overrides = [], photos = [], budget_level = 'Low Budget', production_tier = 'Independent';

CREATE production SET title = 'Winter Crossing', slug = 'winter-crossing', type = 'Film', status = 'Festival',
  description = 'During the final winter of WWII, a German soldier and a Polish refugee must cross the frozen Baltic Sea together to survive.',
  location = 'Berlin, Germany', start_date = d'2024-01-10T00:00:00Z', end_date = d'2025-02-28T00:00:00Z',
  source = 'manual', source_overrides = [], photos = [], budget_level = 'Moderate Budget', production_tier = 'Indie Studio';

CREATE production SET title = 'The Partition', slug = 'the-partition', type = 'TV Series', status = 'Released',
  description = 'Limited series following three families across the 1947 Partition of India. Six episodes of intimate, sweeping drama.',
  location = 'Mumbai, India', start_date = d'2023-09-01T00:00:00Z', end_date = d'2025-01-15T00:00:00Z',
  source = 'manual', source_overrides = [], photos = [], budget_level = 'High Budget', production_tier = 'Studio';

CREATE production SET title = 'Neon Requiem', slug = 'neon-requiem', type = 'Film', status = 'Post-Production',
  description = 'Neo-noir thriller set in rain-soaked Tokyo. A disgraced detective tracks a serial killer through the neon-lit underbelly of the city.',
  location = 'Tokyo, Japan', start_date = d'2025-06-01T00:00:00Z', end_date = d'2026-04-30T00:00:00Z',
  source = 'manual', source_overrides = [], photos = [], budget_level = 'Mid Budget', production_tier = 'Independent';

CREATE production SET title = 'Meridian', slug = 'meridian', type = 'Film', status = 'Pre-Production',
  description = 'Sci-fi drama about the first manned mission to Europa. Intimate character study set against the vastness of space.',
  location = 'Stockholm, Sweden', start_date = d'2026-07-01T00:00:00Z',
  source = 'manual', source_overrides = [], photos = [], budget_level = 'High Budget', production_tier = 'Indie Studio';

CREATE production SET title = 'Backyard Universe', slug = 'backyard-universe', type = 'Documentary', status = 'Production',
  description = 'Feature documentary following amateur astronomers in rural communities across America who are making genuine scientific discoveries from their backyards.',
  location = 'Various, USA',
  source = 'manual', source_overrides = [], photos = [], budget_level = 'Micro Budget', production_tier = 'Independent';

CREATE production SET title = 'The Last Analog', slug = 'the-last-analog', type = 'Short Film', status = 'Completed',
  description = 'A projectionist at the last 35mm cinema in the country faces its final night.',
  location = 'Chicago, IL',
  source = 'manual', source_overrides = [], photos = [], budget_level = 'No Budget', production_tier = 'Student Film';

CREATE production SET title = 'Bloom', slug = 'bloom', type = 'Music Video', status = 'Completed',
  description = 'Music video for indie artist Cleo Park. One-shot floral fantasy sequence.',
  location = 'Los Angeles, CA',
  source = 'manual', source_overrides = [], photos = [], budget_level = 'Micro Budget', production_tier = 'Content Creator';

CREATE production SET title = 'Hollow Ground', slug = 'hollow-ground', type = 'TV Series', status = 'Development',
  description = 'Southern Gothic horror series. A family moves into a plantation home and discovers the land never forgives.',
  location = 'Atlanta, GA',
  source = 'manual', source_overrides = [], photos = [], budget_level = 'Moderate Budget', production_tier = 'Indie Studio';

CREATE production SET title = 'Frostbite', slug = 'frostbite', type = 'Film', status = 'Pre-Production',
  description = 'Arctic survival thriller. A research team is stranded after their station is destroyed by an avalanche. Then things get worse.',
  location = 'Tromsø, Norway', start_date = d'2026-09-01T00:00:00Z',
  source = 'manual', source_overrides = [], photos = [], budget_level = 'Mid Budget', production_tier = 'Independent';

CREATE production SET title = 'Craft Services: The Documentary', slug = 'craft-services-doc', type = 'Documentary', status = 'Production',
  description = 'A loving look at the unsung heroes of every film set: the craft services team.',
  location = 'Los Angeles, CA',
  source = 'manual', source_overrides = [], photos = [], budget_level = 'Micro Budget', production_tier = 'Hobby / Personal';

CREATE production SET title = 'Signal Lost', slug = 'signal-lost', type = 'Webseries', status = 'Canceled',
  description = 'Sci-fi webseries about a radio operator who picks up transmissions from the future. Canceled after funding fell through.',
  location = 'Portland, OR',
  source = 'manual', source_overrides = [], photos = [], budget_level = 'No Budget', production_tier = 'Content Creator';

-- ============================================================================
-- 5. PRODUCTION MEMBERSHIPS
-- ============================================================================

LET $redclay = (SELECT id FROM production WHERE slug = 'red-clay')[0].id;
LET $winter = (SELECT id FROM production WHERE slug = 'winter-crossing')[0].id;
LET $partition = (SELECT id FROM production WHERE slug = 'the-partition')[0].id;
LET $neon = (SELECT id FROM production WHERE slug = 'neon-requiem')[0].id;
LET $meridian = (SELECT id FROM production WHERE slug = 'meridian')[0].id;
LET $backyard = (SELECT id FROM production WHERE slug = 'backyard-universe')[0].id;
LET $lastanalog = (SELECT id FROM production WHERE slug = 'the-last-analog')[0].id;
LET $hollowground = (SELECT id FROM production WHERE slug = 'hollow-ground')[0].id;
LET $frostbite = (SELECT id FROM production WHERE slug = 'frostbite')[0].id;

-- Red Clay
RELATE $marcus->member_of->$redclay SET role = 'owner', production_role = 'Director', invitation_status = 'accepted';
RELATE $sarah->member_of->$redclay SET role = 'member', production_role = 'Cinematographer', invitation_status = 'accepted';
RELATE $sofia->member_of->$redclay SET role = 'member', production_role = 'Casting Director', invitation_status = 'accepted';

-- Winter Crossing
RELATE $hanna->member_of->$winter SET role = 'owner', production_role = 'Producer', invitation_status = 'accepted';
RELATE $chris->member_of->$winter SET role = 'admin', production_role = 'Director', invitation_status = 'accepted';
RELATE $sarah->member_of->$winter SET role = 'member', production_role = 'DP', invitation_status = 'accepted';

-- The Partition
RELATE $priya->member_of->$partition SET role = 'member', production_role = 'Production Designer', invitation_status = 'accepted';
RELATE $kevin->member_of->$partition SET role = 'owner', production_role = 'Executive Producer', invitation_status = 'accepted';

-- Neon Requiem
RELATE $kevin->member_of->$neon SET role = 'owner', production_role = 'Producer', invitation_status = 'accepted';
RELATE $daniel->member_of->$neon SET role = 'member', production_role = 'VFX Supervisor', invitation_status = 'accepted';

-- Meridian
RELATE $rika->member_of->$meridian SET role = 'owner', production_role = 'Line Producer', invitation_status = 'accepted';
RELATE $hanna->member_of->$meridian SET role = 'admin', production_role = 'Producer', invitation_status = 'accepted';
RELATE $daniel->member_of->$meridian SET role = 'member', production_role = 'VFX Supervisor', invitation_status = 'pending';

-- Backyard Universe
RELATE $marcus->member_of->$backyard SET role = 'owner', production_role = 'Director', invitation_status = 'accepted';

-- The Last Analog
RELATE $chris->member_of->$lastanalog SET role = 'owner', production_role = 'Director', invitation_status = 'accepted';

-- Hollow Ground
RELATE $marcus->member_of->$hollowground SET role = 'owner', production_role = 'Creator', invitation_status = 'accepted';
RELATE $kevin->member_of->$hollowground SET role = 'admin', production_role = 'Executive Producer', invitation_status = 'accepted';

-- Frostbite
RELATE $rika->member_of->$frostbite SET role = 'owner', production_role = 'Producer', invitation_status = 'accepted';
RELATE $chris->member_of->$frostbite SET role = 'member', production_role = 'Director', invitation_status = 'pending';

-- Org memberships on productions
RELATE $ember->member_of->$redclay SET role = 'owner', invitation_status = 'accepted';
RELATE $lichtwerk->member_of->$winter SET role = 'admin', production_role = 'Studio Partner', invitation_status = 'accepted';
RELATE $parallax->member_of->$neon SET role = 'admin', production_role = 'VFX Vendor', invitation_status = 'accepted';
RELATE $nordic->member_of->$meridian SET role = 'owner', production_role = 'Lead Production Company', invitation_status = 'accepted';
RELATE $nordic->member_of->$frostbite SET role = 'owner', production_role = 'Production Company', invitation_status = 'accepted';
RELATE $moralescast->member_of->$redclay SET role = 'member', production_role = 'Casting', invitation_status = 'accepted';
RELATE $seceda->member_of->$winter SET role = 'admin', production_role = 'Producer', invitation_status = 'accepted';
RELATE $seceda->member_of->$neon SET role = 'admin', production_role = 'Executive Production', invitation_status = 'accepted';

-- ============================================================================
-- 6. LOCATIONS (8 filming locations)
-- ============================================================================

CREATE location SET
  name = 'Lichtwerk Stage A', address = 'Oberlandstraße 26-35', city = 'Berlin',
  state = 'Berlin', country = 'Germany', postal_code = '12099',
  description = 'Main soundstage. 800 sqm, 12m ceiling height. Full grid, blackout capable. Adjacent to production offices and green rooms.',
  contact_name = 'Hanna Müller', contact_email = 'booking@lichtwerk.studio',
  is_public = true, max_capacity = 200, photos = [],
  amenities = ['WiFi', 'Parking', 'Green Room', 'Production Office', 'Loading Dock', 'Grid'],
  restrictions = ['No pyrotechnics without permit', 'Quiet hours after 10pm'],
  parking_info = 'Private lot with 30 spaces. Truck access via rear gate.',
  created_by = $hanna;

CREATE location SET
  name = 'Red Barn Ranch', address = '4521 Old Mill Road', city = 'Covington',
  state = 'Georgia', country = 'USA', postal_code = '30014',
  description = 'Working ranch with 19th century barn, farmhouse, and 40 acres of rolling fields. Popular for period pieces and Southern Gothic.',
  contact_name = 'Marcus Johnson', contact_email = 'locations@emberfilms.com',
  is_public = true, max_capacity = 50, photos = [],
  amenities = ['Parking', 'Power', 'Running Water'],
  restrictions = ['No drones without prior approval', 'Livestock must be managed'],
  parking_info = 'Gravel lot. Basecamp area available.',
  created_by = $marcus;

CREATE location SET
  name = 'Shoreditch Warehouse', address = '23 Rivington Street', city = 'London',
  state = 'Greater London', country = 'UK', postal_code = 'EC2A 3DU',
  description = 'Raw industrial warehouse space. Exposed brick, concrete floors, massive skylights. Perfect for moody, atmospheric shoots.',
  contact_name = 'Final Frame Post', contact_email = 'space@finalframe.co.uk',
  is_public = true, max_capacity = 100, photos = [],
  amenities = ['WiFi', 'Loading Bay', 'Freight Elevator', 'Blackout Blinds'],
  restrictions = ['No structural modifications', 'Max 2 days booking'],
  created_by = $sarah;

CREATE location SET
  name = 'Shibuya Rooftop', address = '2-24-12 Shibuya', city = 'Tokyo',
  state = 'Tokyo', country = 'Japan', postal_code = '150-0002',
  description = 'Rooftop location overlooking Shibuya Crossing. Night shoots available. Iconic neon backdrop.',
  contact_name = 'Takeshi Yamada', contact_email = 'takeshi@example.com',
  is_public = true, max_capacity = 30, photos = [],
  amenities = ['Power', 'Elevator Access'],
  restrictions = ['Noise restrictions', 'Weather dependent', 'Night permits required'],
  created_by = $kevin;

CREATE location SET
  name = 'Rajasthan Haveli', address = 'Patwon Ki Haveli Road', city = 'Jaisalmer',
  state = 'Rajasthan', country = 'India', postal_code = '345001',
  description = 'Ornate 18th century haveli with sandstone carvings, interior courtyards, and rooftop terraces. Stunning for period and contemporary Indian cinema.',
  contact_name = 'Priya Patel', contact_email = 'priya@example.com',
  is_public = true, max_capacity = 80, photos = [],
  amenities = ['Parking', 'Catering Space', 'Green Room'],
  restrictions = ['Heritage site — no modifications to structure', 'Government permit required'],
  created_by = $priya;

CREATE location SET
  name = 'Vancouver Backlot', address = '8855 Northbrook Court', city = 'Burnaby',
  state = 'BC', country = 'Canada', postal_code = 'V5J 5J3',
  description = 'Exterior backlot with standing New York street set, suburban neighborhood, and alley. Rain towers and wind machines available.',
  contact_name = 'Daniel Okafor', contact_email = 'daniel@example.com',
  is_public = true, max_capacity = 300, photos = [],
  amenities = ['Rain Towers', 'Wind Machines', 'Basecamp Area', 'Power', 'WiFi', 'Parking'],
  parking_info = 'Large paved lot. Truck parking available.',
  created_by = $daniel;

CREATE location SET
  name = 'Fjord Cabin', address = 'Tromsøvegen 12', city = 'Tromsø',
  state = 'Troms', country = 'Norway',
  description = 'Remote cabin on a fjord. Dramatic mountain backdrop, northern lights visible in winter. 2-hour drive from Tromsø airport.',
  contact_name = 'Rika Andersson', contact_email = 'rika@example.com',
  is_public = true, max_capacity = 15, photos = [],
  amenities = ['Generator', 'Running Water', 'Heating'],
  restrictions = ['Weather dependent', 'Limited vehicle access in winter', 'Crew must be self-sufficient'],
  created_by = $rika;

-- Private location (edge case)
CREATE location SET
  name = 'Private Estate', address = '1 Secret Lane', city = 'Beverly Hills',
  state = 'CA', country = 'USA',
  description = 'Luxury estate available for select productions only.',
  contact_name = 'Ghost User', contact_email = 'ghost@example.com',
  is_public = false, photos = [],
  created_by = $kevin;

-- ============================================================================
-- 7. LOCATION RATES
-- ============================================================================

LET $stage_a = (SELECT id FROM location WHERE name = 'Lichtwerk Stage A')[0].id;
LET $barn = (SELECT id FROM location WHERE name = 'Red Barn Ranch')[0].id;
LET $warehouse = (SELECT id FROM location WHERE name = 'Shoreditch Warehouse')[0].id;
LET $backlot = (SELECT id FROM location WHERE name = 'Vancouver Backlot')[0].id;

CREATE location_rate SET location = $stage_a, rate_type = 'daily', amount = 3500, currency = 'EUR', description = 'Full day (12 hours)';
CREATE location_rate SET location = $stage_a, rate_type = 'weekly', amount = 15000, currency = 'EUR', description = '5-day week', minimum_duration = 5;
CREATE location_rate SET location = $barn, rate_type = 'daily', amount = 1200, currency = 'USD', description = 'Full property access';
CREATE location_rate SET location = $warehouse, rate_type = 'daily', amount = 2800, currency = 'GBP', description = 'Full warehouse';
CREATE location_rate SET location = $warehouse, rate_type = 'hourly', amount = 400, currency = 'GBP', description = 'Minimum 4 hours', minimum_duration = 4;
CREATE location_rate SET location = $backlot, rate_type = 'daily', amount = 5000, currency = 'CAD', description = 'Full backlot access';
CREATE location_rate SET location = $backlot, rate_type = 'weekly', amount = 20000, currency = 'CAD', description = 'Monday–Friday', minimum_duration = 5;

-- ============================================================================
-- 8. LIKES (various social connections)
-- ============================================================================

RELATE $chris->likes->$sarah;
RELATE $chris->likes->$marcus;
RELATE $kevin->likes->$sarah;
RELATE $kevin->likes->$priya;
RELATE $sarah->likes->$marcus;
RELATE $sarah->likes->$chris;
RELATE $marcus->likes->$sarah;
RELATE $marcus->likes->$chris;
RELATE $hanna->likes->$chris;
RELATE $hanna->likes->$rika;
RELATE $rika->likes->$hanna;
RELATE $daniel->likes->$sarah;

LET $emilia = (SELECT id FROM person WHERE username = 'emiliarossi')[0].id;
RELATE $chris->likes->$emilia;
RELATE $kevin->likes->$emilia;
RELATE $emilia->likes->$chris;

-- Like locations
RELATE $chris->likes->$stage_a;
RELATE $chris->likes->$barn;
RELATE $sarah->likes->$warehouse;
RELATE $kevin->likes->$backlot;
RELATE $marcus->likes->$barn;

-- ============================================================================
-- 9. JOB POSTINGS (10 diverse jobs)
-- ============================================================================

LET $craftcatering = (SELECT id FROM organization WHERE slug = 'craft-catering')[0].id;

CREATE job_posting SET
  title = 'Cinematographer — "Frostbite"', description = 'Seeking an experienced cinematographer for arctic survival thriller "Frostbite." Must be comfortable shooting in extreme cold conditions (-20°C). 6-week principal photography in Tromsø, Norway. ARRI Alexa Mini LF package provided. Union rates.',
  location = 'Tromsø, Norway', posted_by = $rika, contact_name = 'Rika Andersson', contact_email = 'rika@example.com',
  status = 'open', expires_at = d'2026-06-01T00:00:00Z', applications_enabled = true,
  roles = [{ title: 'Cinematographer / DP', description: 'Lead DP for arctic thriller', rate_type: 'Weekly', rate_amount: '8500' }];

CREATE job_posting SET
  title = 'Multiple Crew Positions — "Meridian"', description = 'Nordic Light Films is crewing up for sci-fi feature "Meridian." Multiple positions available across departments. Virtual production experience preferred.',
  location = 'Stockholm, Sweden', posted_by = $nordic, contact_name = 'Rika Andersson', contact_email = 'rika@example.com',
  status = 'open', expires_at = d'2026-08-15T00:00:00Z', applications_enabled = true,
  roles = [
    { title: '1st AD', description: 'Experienced 1st AD for 40-day shoot', rate_type: 'Weekly', rate_amount: '5000' },
    { title: 'Script Supervisor', description: 'Meticulous script supervisor needed', rate_type: 'Weekly', rate_amount: '3500' },
    { title: 'Key Grip', description: 'Key grip with VP stage experience', rate_type: 'Weekly', rate_amount: '4000' }
  ];

CREATE job_posting SET
  title = 'VFX Artists — Parallax VFX', description = 'Growing team seeks mid-level and senior compositors and CG generalists. Remote-friendly with occasional on-site in Vancouver. Nuke and Houdini proficiency required.',
  location = 'Vancouver, BC (Remote OK)', posted_by = $daniel, contact_email = 'jobs@parallax.vfx',
  status = 'open', expires_at = d'2026-12-31T00:00:00Z', applications_enabled = true,
  roles = [
    { title: 'Senior Compositor', description: 'Nuke. 5+ years feature film.', rate_type: 'Day Rate', rate_amount: '650' },
    { title: 'CG Generalist', description: 'Houdini/Maya. Environments focus.', rate_type: 'Day Rate', rate_amount: '600' }
  ];

CREATE job_posting SET
  title = 'Actors — "Hollow Ground" Table Read', description = 'Seeking actors for paid table read of Southern Gothic horror pilot "Hollow Ground." Diverse cast, multiple speaking roles.',
  location = 'Atlanta, GA', posted_by = $marcus, contact_email = 'casting@emberfilms.com',
  status = 'open', expires_at = d'2026-05-01T00:00:00Z', applications_enabled = true,
  roles = [
    { title: 'Lead — Mama June', description: 'Female, 55-65. Formidable matriarch.', rate_type: 'Flat', rate_amount: '2000' },
    { title: 'Lead — Delia', description: 'Female, 28-35. Returning daughter.', rate_type: 'Flat', rate_amount: '2000' },
    { title: 'Supporting — Pastor Hollis', description: 'Male, 45-60. Charismatic preacher.', rate_type: 'Flat', rate_amount: '1000' }
  ];

CREATE job_posting SET
  title = 'Production Assistant — Ember Films', description = 'Entry-level PA position. Must have valid driver license and be available for early call times. Great learning opportunity on an active indie production company.',
  location = 'Atlanta, GA', posted_by = $ember, contact_email = 'jobs@emberfilms.com',
  status = 'open', expires_at = d'2026-04-30T00:00:00Z', applications_enabled = true,
  roles = [{ title: 'Production Assistant', description: 'General PA duties', rate_type: 'Day Rate', rate_amount: '200' }];

CREATE job_posting SET
  title = 'Sound Mixer — Documentary', description = 'Need a production sound mixer for 3-day documentary shoot in rural USA. Must own kit. Travel and per diem covered.',
  location = 'Various, USA', posted_by = $marcus,
  status = 'open', expires_at = d'2026-05-15T00:00:00Z', applications_enabled = true,
  roles = [{ title: 'Production Sound Mixer', description: 'Own kit required. Boom op provided.', rate_type: 'Day Rate', rate_amount: '750' }];

-- Closed job (edge case)
CREATE job_posting SET
  title = 'Editor — "Red Clay" (FILLED)', description = 'Position has been filled. Thank you to all applicants.',
  location = 'Atlanta, GA', posted_by = $marcus,
  status = 'filled', expires_at = d'2025-06-01T00:00:00Z', applications_enabled = false,
  roles = [{ title: 'Editor', description: 'Feature film editor', rate_type: 'Flat', rate_amount: '15000' }];

-- Expired job (edge case)
CREATE job_posting SET
  title = 'Intern — Lichtwerk Studio', description = 'Summer internship program at Lichtwerk Studio. This posting has expired.',
  location = 'Berlin, Germany', posted_by = $hanna,
  status = 'closed', expires_at = d'2025-09-01T00:00:00Z', applications_enabled = false,
  roles = [{ title: 'Studio Intern', description: 'Unpaid internship, academic credit available', rate_type: 'Unpaid' }];

-- Job posted by org (edge case)
CREATE job_posting SET
  title = 'Craft Services Lead', description = 'Craft Catering is hiring a lead for an upcoming 8-week shoot. Must handle dietary restrictions and crew of 80+.',
  location = 'Los Angeles, CA', posted_by = $craftcatering,
  status = 'open', expires_at = d'2026-07-01T00:00:00Z', applications_enabled = true,
  roles = [{ title: 'Craft Services Lead', description: 'Lead craft services for 80+ crew', rate_type: 'Weekly', rate_amount: '2800' }];

-- No-contact job (edge case: minimal info)
CREATE job_posting SET
  title = 'Camera Operator Needed ASAP', description = 'Quick turnaround commercial shoot. 2 days. DM for details.',
  location = 'New York, NY', posted_by = $kevin,
  status = 'open', expires_at = d'2026-04-01T00:00:00Z', applications_enabled = false,
  roles = [{ title: 'Camera Operator', rate_type: 'TBD' }];

-- ============================================================================
-- 10. CONVERSATIONS & MESSAGES
-- ============================================================================

LET $conv1 = (CREATE conversation SET participant_a = $chris, participant_b = $sarah, last_message_at = time::now())[0].id;
CREATE direct_message SET conversation = $conv1, sender = $chris, body = 'Hey Sarah, loved your work on that Sundance short. Would you be interested in DP-ing a feature in Berlin this fall?';
CREATE direct_message SET conversation = $conv1, sender = $sarah, body = 'Chris! Thanks so much. Would love to hear more about the project. What is the timeline looking like?';
CREATE direct_message SET conversation = $conv1, sender = $chris, body = 'Looking at a 6-week shoot starting late September. Will send you the lookbook this week.';

LET $conv2 = (CREATE conversation SET participant_a = $hanna, participant_b = $rika, last_message_at = time::now())[0].id;
CREATE direct_message SET conversation = $conv2, sender = $hanna, body = 'Rika, the Meridian budget came back from the Swedish Film Institute. We got the full amount!';
CREATE direct_message SET conversation = $conv2, sender = $rika, body = 'That is incredible news!! Time to start crewing up. Will draft the schedule this weekend.';

LET $conv3 = (CREATE conversation SET participant_a = $kevin, participant_b = $marcus, last_message_at = time::now())[0].id;
CREATE direct_message SET conversation = $conv3, sender = $marcus, body = 'Kevin, just wanted to follow up on the Hollow Ground pitch. Did the network respond yet?';
CREATE direct_message SET conversation = $conv3, sender = $kevin, body = 'Still waiting. They said end of week. Fingers crossed.';

-- ============================================================================
-- 11. NOTIFICATIONS
-- ============================================================================

CREATE notification SET person_id = $sarah, notification_type = 'invitation', title = 'Production Invitation', message = 'You have been invited to join "Meridian" as Cinematographer.', link = '/productions/meridian', read = false;
CREATE notification SET person_id = $sarah, notification_type = 'message', title = 'New Message', message = 'Chris sent you a message.', link = '/messages', read = true;
CREATE notification SET person_id = $marcus, notification_type = 'job_application', title = 'New Application', message = 'Someone applied to your "Hollow Ground" casting call.', link = '/jobs', read = false;
CREATE notification SET person_id = $chris, notification_type = 'invitation_accepted', title = 'Invitation Accepted', message = 'Sarah Chen accepted your invitation to "Winter Crossing".', link = '/productions/winter-crossing', read = true;
CREATE notification SET person_id = $kevin, notification_type = 'general', title = 'Welcome to SlateHub', message = 'Thanks for joining! Complete your profile to get discovered.', read = true;
CREATE notification SET person_id = $daniel, notification_type = 'member_joined', title = 'New Team Member', message = 'A new member joined Parallax VFX.', link = '/orgs/parallax-vfx', read = false;
CREATE notification SET person_id = $rika, notification_type = 'message', title = 'New Message', message = 'Hanna Müller sent you a message about Meridian.', link = '/messages', read = false;
CREATE notification SET person_id = $hanna, notification_type = 'general', title = 'Profile Views', message = 'Your profile was viewed 12 times this week.', read = false;

-- ============================================================================
-- 12. PROFILE VIEWS (analytics data)
-- ============================================================================

CREATE profile_view SET profile_id = $chris, viewer_id = $sarah, referrer = '/people', referrer_source = 'browse', viewed_at = d'2026-03-10T14:30:00Z';
CREATE profile_view SET profile_id = $chris, viewer_id = $marcus, referrer = '/search?q=director', referrer_source = 'search', viewed_at = d'2026-03-11T09:15:00Z';
CREATE profile_view SET profile_id = $chris, viewer_id = $hanna, referrer = '/productions/winter-crossing', referrer_source = 'production', viewed_at = d'2026-03-12T16:45:00Z';
CREATE profile_view SET profile_id = $chris, viewer_id = NONE, referrer = 'https://google.com', referrer_source = 'external', viewed_at = d'2026-03-13T11:00:00Z';
CREATE profile_view SET profile_id = $chris, viewer_id = $kevin, referrer = '/people', referrer_source = 'browse', viewed_at = d'2026-03-14T08:30:00Z';
CREATE profile_view SET profile_id = $sarah, viewer_id = $chris, referrer = '/search?q=cinematographer', referrer_source = 'search', viewed_at = d'2026-03-10T10:00:00Z';
CREATE profile_view SET profile_id = $sarah, viewer_id = $marcus, referrer = '/people', referrer_source = 'browse', viewed_at = d'2026-03-11T14:20:00Z';
CREATE profile_view SET profile_id = $sarah, viewer_id = NONE, referrer = 'https://linkedin.com', referrer_source = 'external', viewed_at = d'2026-03-15T07:00:00Z';
CREATE profile_view SET profile_id = $marcus, viewer_id = $chris, referrer = '/people', referrer_source = 'browse', viewed_at = d'2026-03-12T11:00:00Z';
CREATE profile_view SET profile_id = $marcus, viewer_id = $kevin, referrer = '/search?q=writer+atlanta', referrer_source = 'search', viewed_at = d'2026-03-14T15:30:00Z';
CREATE profile_view SET profile_id = $emilia, viewer_id = $sofia, referrer = '/people', referrer_source = 'browse', viewed_at = d'2026-03-16T09:00:00Z';
CREATE profile_view SET profile_id = $emilia, viewer_id = $chris, referrer = '/search?q=actor+london', referrer_source = 'search', viewed_at = d'2026-03-16T10:30:00Z';

-- ============================================================================
-- 13. JOB APPLICATIONS
-- ============================================================================

LET $frostbite_job = (SELECT id FROM job_posting WHERE title = 'Cinematographer — "Frostbite"')[0].id;
LET $hollowground_job = (SELECT id FROM job_posting WHERE title = 'Actors — "Hollow Ground" Table Read')[0].id;
LET $pa_job = (SELECT id FROM job_posting WHERE title = 'Production Assistant — Ember Films')[0].id;

RELATE $sarah->application->$frostbite_job SET role_title = 'Cinematographer / DP', cover_letter = 'I have extensive experience shooting in challenging conditions and would love to bring my naturalistic style to this project. My Sundance work was shot largely in cold weather environments.', status = 'shortlisted';
RELATE $emilia->application->$hollowground_job SET role_title = 'Lead — Delia', cover_letter = 'The character of Delia resonates deeply with me. Her journey of returning home to confront the past is something I can bring real emotional depth to.', status = 'submitted';
LET $jackflynn = (SELECT id FROM person WHERE username = 'jackflynn')[0].id;
LET $ghostuser = (SELECT id FROM person WHERE username = 'ghostuser')[0].id;
RELATE $jackflynn->application->$hollowground_job SET role_title = 'Supporting — Pastor Hollis', cover_letter = 'While primarily known for action work, I trained in classical theatre and have been looking for character-driven material.', status = 'reviewed';
RELATE $ghostuser->application->$pa_job SET role_title = 'Production Assistant', status = 'submitted';

-- ============================================================================
-- 14. FEEDBACK
-- ============================================================================

CREATE feedback SET username = 'sarah', page_url = '/productions/red-clay', message = 'Would be great to have a way to upload BTS photos directly to the production page.';
CREATE feedback SET username = 'anonymous', page_url = '/people', message = 'The search could be faster. Sometimes it takes a few seconds to return results.';
CREATE feedback SET username = 'marcus', page_url = '/jobs', message = 'Love the new jobs feature! Would be nice to filter by pay type.';

-- ============================================================================
-- DONE
-- ============================================================================
