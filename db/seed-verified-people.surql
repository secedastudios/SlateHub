-- Seed: real verified people (identity-verified, with avatars) for local dev.
--
-- Names, headlines, and avatar URLs are pulled from real public profiles on
-- the production site (https://slatehub.com). Avatars reference production
-- media directly (absolute URLs) so they render without copying binaries into
-- the repo. These rows are `verification_status = 'identity'` with a photo, so
-- they populate the verified-profile carousel + hero avatars on the `/a/*`
-- landing pages (which require both). Imported by `make db-seed`.
--
-- One statement per line (the `surreal sql` REPL parses line-by-line).
-- Password for all: pass123. Emails are synthetic dev addresses.

-- tom
CREATE person SET username = 'tom', email = 'tom@seed.slatehub.dev', password = crypto::argon2::generate('pass123'), name = 'Tom Gottschalk', verification_status = 'identity', profile = { name: 'Tom Gottschalk', headline: 'Cinematographer', avatar: 'https://slatehub.com/api/media/profiles/dj03mga50rm3lw6c8r14/01KMGCJ4ZBKJEZMTQZFJG8JA0P.jpg', bio: 'Berlin-based cinematographer into documentaries and YouTube. Part of Seceda Studios.', location: 'Berlin, Germany', is_public: true, skills: ['Cinematography', 'Video Editing', 'Camera Assistant'], languages: ['German', 'English'], ethnicity: [], unions: [], social_links: [], reels: [], media_other: [], education: [], awards: [], photos: [] };

-- renjith
CREATE person SET username = 'renjith', email = 'renjith@seed.slatehub.dev', password = crypto::argon2::generate('pass123'), name = 'Renjith Radhakrishnan', verification_status = 'identity', profile = { name: 'Renjith Radhakrishnan', headline: 'Cinematographer | 1st AC', avatar: 'https://slatehub.com/api/media/profiles/kqvs6uk63nqnq44vzfn7/01KPZZ36212TP5XHX2ZEM418FA.jpg', bio: 'Cinematographer and first assistant camera based in Berlin.', location: 'Berlin, Germany', is_public: true, skills: ['Cinematography', 'Color Grading', 'Camera Assistant'], languages: ['English', 'Malayalam', 'Hindi'], ethnicity: [], unions: [], social_links: [], reels: [], media_other: [], education: [], awards: [], photos: [] };

-- simonschreiner
CREATE person SET username = 'simonschreiner', email = 'simon@seed.slatehub.dev', password = crypto::argon2::generate('pass123'), name = 'Simon Schreiner', verification_status = 'identity', profile = { name: 'Simon Schreiner', headline: 'Producer / Writer / Cinematographer', avatar: 'https://slatehub.com/api/media/profiles/28fbg2kttx5zeqscoc30/01KQTKDTSSWEP4Z75PCJC38766.jpg', bio: 'Independent film producer and writer with a business and marketing background. Host of the European Filmmakers community.', location: 'Berlin, Germany', is_public: true, skills: ['Screenwriting', 'Cinematography', 'Film Production'], languages: ['German', 'English', 'Russian', 'French'], ethnicity: [], unions: [], social_links: [], reels: [], media_other: [], education: [], awards: [], photos: [] };

-- maks
CREATE person SET username = 'maks', email = 'maks@seed.slatehub.dev', password = crypto::argon2::generate('pass123'), name = 'Maks Pallas', verification_status = 'identity', profile = { name: 'Maks Pallas', headline: 'Cinematographer & Director', avatar: 'https://slatehub.com/api/media/profiles/z52uw0ld3b7w5plfccz6/01KRXQH9JPB0EENMTGNBS3RNHE.jpg', bio: 'Film director, cinematographer and editor based in Dresden. Favors sociocultural and political projects.', location: 'Dresden & Leipzig', is_public: true, skills: ['Cinematography', 'Video Editing', 'Color Grading', 'Production Design'], languages: ['English', 'German'], ethnicity: [], unions: [], social_links: [], reels: [], media_other: [], education: [], awards: [], photos: [] };

-- moritz
CREATE person SET username = 'moritz', email = 'moritz@seed.slatehub.dev', password = crypto::argon2::generate('pass123'), name = 'Moritz Erdt', verification_status = 'identity', profile = { name: 'Moritz Erdt', headline: 'Director | DOP', avatar: 'https://slatehub.com/api/media/profiles/am6yeb6kjl0bkq4gqv1e/01KN70K55MCXARA8AJCKE0H4R2.jpg', bio: 'Berlin-based filmmaker and founder of Filmmakers & Movie Nerds, a community built to help creatives start creating.', location: 'Berlin, Germany', is_public: true, skills: ['Directing', 'Cinematography', 'Storytelling'], languages: ['German', 'English'], ethnicity: [], unions: [], social_links: [], reels: [], media_other: [], education: [], awards: [], photos: [] };

-- cineastmjp
CREATE person SET username = 'cineastmjp', email = 'marcel@seed.slatehub.dev', password = crypto::argon2::generate('pass123'), name = 'Marcel Janick Paul', verification_status = 'identity', profile = { name: 'Marcel Janick Paul', headline: 'Film Editor | Director | Producer', avatar: 'https://slatehub.com/api/media/profiles/43qikxgsshkz9w2b3jtv/01KM0FMF76449DAPGFSK2Q5VKX.jpg', bio: 'Young filmmaker and cineaste based in Frankfurt and Berlin. Works mainly in editing, directing, and writing.', location: 'Berlin, Germany', is_public: true, skills: ['Film Editing', 'Directing', 'Producing', 'Writing', 'Camera Operating'], languages: ['German', 'English', 'French', 'Thai'], ethnicity: [], unions: [], social_links: [], reels: [], media_other: [], education: [], awards: [], photos: [] };

-- leo
CREATE person SET username = 'leo', email = 'eleonora@seed.slatehub.dev', password = crypto::argon2::generate('pass123'), name = 'Eleonora Jaremko', verification_status = 'identity', profile = { name: 'Eleonora Jaremko', headline: 'Editor & Colorist', avatar: 'https://slatehub.com/api/media/profiles/7hm9j6hbrju2zqhhm9az/01KSSDYC483YTG8GQDRR7M9CD5.jpg', bio: 'Editor and colorist working in videography and post-production.', location: 'Hungary', is_public: true, skills: ['Video Editing', 'Videography', 'Color Grading'], languages: ['English', 'Hungarian'], ethnicity: [], unions: [], social_links: [], reels: [], media_other: [], education: [], awards: [], photos: [] };
