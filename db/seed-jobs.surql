-- Seed 100 realistic job postings
-- Uses chris (person) and Seceda (organization) as posters
-- Requires db-seed to have run first

LET $chris = (SELECT id FROM person WHERE username = 'chris')[0].id;
LET $seceda = (SELECT id FROM organization WHERE slug = 'seceda')[0].id;

-- 1
CREATE job_posting SET title = 'Director of Photography', description = 'Seeking an experienced DP for a character-driven indie feature shooting over 4 weeks in the Scottish Highlands. Must be comfortable with natural light and handheld work. Arri Alexa Mini provided.', location = 'Edinburgh, UK', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Director of Photography', description: 'Lead cinematographer for 28-day shoot', rate_type: 'Day Rate', rate_amount: '€650/day' }];

-- 2
CREATE job_posting SET title = 'Short Film — Lead Actress', description = 'Casting a female lead (25-35) for a 15-minute drama about grief and reconciliation. Non-union. Shooting over one weekend in Berlin. Self-tapes accepted.', location = 'Berlin, Germany', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Lead Actress', description: 'Female, 25-35, German or English speaking', rate_type: 'Flat', rate_amount: '€500' }];

-- 3
CREATE job_posting SET title = 'Production Assistant — TV Series', description = 'Fast-paced crime drama needs 2 PAs for Season 3. Must have valid driving license and be available for night shoots. Prior set experience preferred.', location = 'Dublin, Ireland', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Production Assistant', description: 'General PA duties, driving, set logistics', rate_type: 'Day Rate', rate_amount: '€180/day' }, { title: 'Office PA', description: 'Production office support, scheduling, petty cash', rate_type: 'Day Rate', rate_amount: '€160/day' }];

-- 4
CREATE job_posting SET title = 'Sound Recordist & Boom Operator', description = 'Looking for a sound team for a documentary shoot following street musicians across three European cities. Travel expenses covered. Own kit preferred.', location = 'Paris, France', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Sound Recordist', description: 'Location sound recording, must own Sound Devices mixer', rate_type: 'Day Rate', rate_amount: '€500/day' }, { title: 'Boom Operator', description: 'Boom operation, radio mic management', rate_type: 'Day Rate', rate_amount: '€300/day' }];

-- 5
CREATE job_posting SET title = 'VFX Compositor — Feature Film', description = 'Post-production house seeks Nuke compositors for a sci-fi feature. Remote work available. 3-month contract starting April. Showreel required.', location = 'Remote', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Senior Compositor', description: '5+ years Nuke experience, CG integration', rate_type: 'Day Rate', rate_amount: '€550/day' }, { title: 'Junior Compositor', description: '1-2 years experience, roto and cleanup', rate_type: 'Day Rate', rate_amount: '€300/day' }, { title: 'Prep Artist', description: 'Plate prep, paint and roto', rate_type: 'Day Rate', rate_amount: '€250/day' }];

-- 6
CREATE job_posting SET title = 'Gaffer — Commercial Shoot', description = 'Luxury fashion brand commercial. 3-day shoot in a historic villa outside Rome. Must bring own lighting package or have access to rental house.', location = 'Rome, Italy', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 7d, roles = [{ title: 'Gaffer', description: 'Lighting design and execution', rate_type: 'Day Rate', rate_amount: '€600/day' }];

-- 7
CREATE job_posting SET title = 'Casting Director — Period Drama', description = 'BBC-commissioned 6-part period drama set in 1920s London. Seeking casting director with experience in period pieces. Must be based in or near London.', location = 'London, UK', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Casting Director', description: 'Full casting for 6-episode series, principals and supporting', rate_type: 'Flat', rate_amount: '€15,000' }];

-- 8
CREATE job_posting SET title = 'Editor — Music Video', description = 'Hip-hop music video shot on RED. Quick turnaround needed — 5 working days from delivery of footage. DaVinci Resolve preferred. Must sign NDA.', location = 'Remote', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 7d, roles = [{ title: 'Editor', description: 'Offline and online edit, color grade', rate_type: 'Flat', rate_amount: '€2,000' }];

-- 9
CREATE job_posting SET title = 'Stunt Coordinator — Action Feature', description = 'Low-budget action thriller shooting in Prague. Fight choreography, car work, and wire rigs. Insurance and risk assessment experience mandatory.', location = 'Prague, Czech Republic', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Stunt Coordinator', description: 'Design and supervise all action sequences', rate_type: 'Weekly', rate_amount: '€4,000/week' }, { title: 'Stunt Performer', description: 'Fight and fall work, martial arts background preferred', rate_type: 'Day Rate', rate_amount: '€400/day' }];

-- 10
CREATE job_posting SET title = 'Production Designer — Feature Film', description = 'Art-house drama filming in abandoned industrial spaces. Minimal budget, maximum creativity. Seeking someone who can build atmosphere from nothing.', location = 'Hamburg, Germany', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Production Designer', description: 'Full design and art direction', rate_type: 'Flat', rate_amount: '€5,000' }];

-- 11
CREATE job_posting SET title = 'Screenwriter — Horror Anthology', description = 'Streaming platform commissioning 8 standalone horror stories, 20 minutes each. Writers submit spec scripts for consideration. Diverse voices encouraged.', location = 'Remote', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Screenwriter', description: 'Original 20-minute horror screenplay', rate_type: 'Flat', rate_amount: '€3,000 per script' }];

-- 12
CREATE job_posting SET title = '1st Assistant Director', description = 'Experienced 1st AD needed for a 6-week feature shoot. Complex scheduling with multiple locations, night exteriors, and crowd scenes. Must speak English and French.', location = 'Brussels, Belgium', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: '1st Assistant Director', description: 'Scheduling, set management, crew coordination', rate_type: 'Weekly', rate_amount: '€2,500/week' }];

-- 13
CREATE job_posting SET title = 'Costume Designer & Wardrobe Team', description = 'Romantic comedy set in contemporary Milan. 30-day shoot. Need full wardrobe department. Access to Italian fashion houses a plus.', location = 'Milan, Italy', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Costume Designer', description: 'Design and source all costumes', rate_type: 'Weekly', rate_amount: '€2,000/week' }, { title: 'Wardrobe Supervisor', description: 'On-set wardrobe management, continuity', rate_type: 'Day Rate', rate_amount: '€250/day' }, { title: 'Wardrobe Assistant', description: 'Dressing, steaming, costume maintenance', rate_type: 'Day Rate', rate_amount: '€150/day' }];

-- 14
CREATE job_posting SET title = 'Drone Operator — Nature Documentary', description = 'Wildlife documentary series needs licensed drone operators for aerial cinematography. Various locations across Scandinavia. Must hold valid EU drone license.', location = 'Oslo, Norway', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Drone Operator', description: 'Aerial cinematography, own DJI Inspire or equivalent', rate_type: 'Day Rate', rate_amount: '€800/day' }];

-- 15
CREATE job_posting SET title = 'Line Producer — Independent Feature', description = 'Seeking a line producer to manage a €800K budget for a coming-of-age drama. Pre-production starts immediately. Greek co-production.', location = 'Athens, Greece', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Line Producer', description: 'Budget management, crew hiring, logistics', rate_type: 'Flat', rate_amount: '€12,000' }];

-- 16
CREATE job_posting SET title = 'Colorist — Short Film Festival Submissions', description = 'Three short films need color grading before festival deadlines. Shot on Blackmagic RAW. DaVinci Resolve Studio. Remote delivery.', location = 'Remote', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 7d, roles = [{ title: 'Colorist', description: 'Grade 3 short films, 10-15 min each', rate_type: 'Flat', rate_amount: '€1,500 total' }];

-- 17
CREATE job_posting SET title = 'Background Actors — Historical Epic', description = 'Large-scale historical production needs background performers for battle and market scenes. No experience necessary. Fittings required in advance.', location = 'Budapest, Hungary', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Background Actor — Soldiers', description: 'Male, 18-45, physically fit, available 5 consecutive days', rate_type: 'Day Rate', rate_amount: '€120/day' }, { title: 'Background Actor — Market Villagers', description: 'All ages, any gender', rate_type: 'Day Rate', rate_amount: '€100/day' }];

-- 18
CREATE job_posting SET title = 'Sound Designer — Animation', description = 'Original animated series (10 x 11 min) needs creative sound design. Foley, ambience, and FX. Pro Tools required. Remote collaboration.', location = 'Remote', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Sound Designer', description: 'Full sound design for 10-episode animated series', rate_type: 'Flat', rate_amount: '€8,000' }];

-- 19
CREATE job_posting SET title = 'Focus Puller — Feature Film', description = 'Anamorphic shoot on Panavision. 5-week principal photography. Experienced 1st AC with anamorphic experience essential. Own tools expected.', location = 'Vienna, Austria', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: '1st AC / Focus Puller', description: 'Anamorphic lens experience required', rate_type: 'Day Rate', rate_amount: '€400/day' }];

-- 20
CREATE job_posting SET title = 'Makeup & Hair Team — Period Drama', description = 'Six-part series set in 1890s Vienna. Wigs, prosthetics, and period-accurate styling. Must be available for full 8-week shoot.', location = 'Vienna, Austria', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Head of Makeup', description: 'Period makeup design and application', rate_type: 'Weekly', rate_amount: '€2,200/week' }, { title: 'Hair Stylist', description: 'Period hairstyling, wig maintenance', rate_type: 'Weekly', rate_amount: '€1,800/week' }, { title: 'Prosthetics Artist', description: 'Age makeups and special effects', rate_type: 'Day Rate', rate_amount: '€500/day' }];

-- 21
CREATE job_posting SET title = 'Location Manager — TV Pilot', description = 'Tech thriller pilot needs locations in and around Lisbon. Mix of modern offices, server rooms, and residential. Permit experience in Portugal required.', location = 'Lisbon, Portugal', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Location Manager', description: 'Scout, negotiate, and manage all locations', rate_type: 'Weekly', rate_amount: '€1,800/week' }];

-- 22
CREATE job_posting SET title = 'Script Supervisor — Feature Film', description = 'Continuity-heavy drama with non-linear timeline. Script supervisor must track scene connections across multiple timelines. Strong attention to detail.', location = 'Munich, Germany', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Script Supervisor', description: 'Continuity, timing, script notes', rate_type: 'Day Rate', rate_amount: '€350/day' }];

-- 23
CREATE job_posting SET title = 'Documentary Crew — Climate Change Series', description = 'International documentary series on climate adaptation. Shooting across 6 countries over 4 months. Self-contained crews preferred.', location = 'Various — Europe', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Documentary DP', description: 'Vérité style, own camera package preferred', rate_type: 'Day Rate', rate_amount: '€600/day' }, { title: 'Field Producer', description: 'Research, access, on-location producing', rate_type: 'Day Rate', rate_amount: '€450/day' }];

-- 24
CREATE job_posting SET title = 'Grip — Commercial Block', description = 'Three back-to-back car commercials for German automotive brand. Studio and location work. Dolly, crane, and process trailer experience needed.', location = 'Stuttgart, Germany', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 7d, roles = [{ title: 'Key Grip', description: 'Rigging, dolly, process trailer', rate_type: 'Day Rate', rate_amount: '€450/day' }, { title: 'Best Boy Grip', description: 'Assist key grip, manage grip equipment', rate_type: 'Day Rate', rate_amount: '€300/day' }];

-- 25
CREATE job_posting SET title = 'Composer — Feature Film Score', description = 'Psychological thriller needs an original orchestral score. Temp track references: Jonny Greenwood, Mica Levi. Budget for small ensemble recording included.', location = 'Remote', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Composer', description: 'Original score, orchestral and electronic hybrid', rate_type: 'Flat', rate_amount: '€20,000' }];

-- 26
CREATE job_posting SET title = 'Set Decorator — Boutique Hotel Ad', description = 'Luxury hotel chain campaign. 2-day shoot in a converted palazzo. Need someone who can source high-end props and create aspirational interiors.', location = 'Florence, Italy', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 7d, roles = [{ title: 'Set Decorator', description: 'Props sourcing, set dressing for luxury aesthetic', rate_type: 'Day Rate', rate_amount: '€400/day' }];

-- 27
CREATE job_posting SET title = 'Post-Production Supervisor', description = 'Managing post workflow for a 90-minute documentary. Offline in Premiere, online in DaVinci. Coordinating sound mix, grade, and deliverables.', location = 'Amsterdam, Netherlands', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Post-Production Supervisor', description: 'Manage full post pipeline from offline to delivery', rate_type: 'Flat', rate_amount: '€6,000' }];

-- 28
CREATE job_posting SET title = 'Steadicam Operator — Dance Film', description = 'Contemporary dance film shot entirely in one take. Steadicam or gimbal expertise essential. Rehearsal days included in schedule.', location = 'Copenhagen, Denmark', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Steadicam Operator', description: 'Single-take dance sequences, own rig preferred', rate_type: 'Day Rate', rate_amount: '€700/day' }];

-- 29
CREATE job_posting SET title = 'Unit Production Manager', description = 'Feature film shooting across 3 countries. Need a UPM with international production experience, carnet knowledge, and multi-currency budgeting.', location = 'Berlin, Germany', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Unit Production Manager', description: 'Budget, scheduling, cross-border logistics', rate_type: 'Weekly', rate_amount: '€3,000/week' }];

-- 30
CREATE job_posting SET title = 'DIT — High-End TV Series', description = 'Premium streaming series needs a DIT for on-set color management, dailies, and data wrangling. ACES workflow. 10-week commitment.', location = 'Madrid, Spain', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'DIT', description: 'On-set color, dailies processing, ACES pipeline', rate_type: 'Day Rate', rate_amount: '€500/day' }, { title: 'Data Wrangler', description: 'Media management, backup verification', rate_type: 'Day Rate', rate_amount: '€200/day' }];

-- 31
CREATE job_posting SET title = 'Intimacy Coordinator', description = 'Feature film with several intimate scenes requires an experienced intimacy coordinator. Must be certified. 2 weeks of rehearsal + 1 week shooting.', location = 'Stockholm, Sweden', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Intimacy Coordinator', description: 'Choreograph and supervise intimate scenes', rate_type: 'Day Rate', rate_amount: '€500/day' }];

-- 32
CREATE job_posting SET title = 'Props Master — Sci-Fi Series', description = 'Fabrication-heavy sci-fi series needs a props master who can design and build futuristic gadgets, weapons, and set pieces. Workshop access provided.', location = 'Prague, Czech Republic', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Props Master', description: 'Design and fabricate custom sci-fi props', rate_type: 'Weekly', rate_amount: '€2,000/week' }, { title: 'Props Maker', description: 'Build, paint, and finish prop pieces', rate_type: 'Day Rate', rate_amount: '€250/day' }];

-- 33
CREATE job_posting SET title = 'Casting — Young Actors for Coming-of-Age Film', description = 'Open casting call for teenagers (14-17) for a Berlinale-targeted coming-of-age drama. No prior acting experience required. Parents must consent.', location = 'Berlin, Germany', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Lead — Boy (14-17)', description: 'Must speak German fluently, no experience needed', rate_type: 'Flat', rate_amount: '€2,000' }, { title: 'Lead — Girl (14-17)', description: 'Must speak German fluently, no experience needed', rate_type: 'Flat', rate_amount: '€2,000' }, { title: 'Supporting — Teacher', description: 'Male, 40-55, professional actor preferred', rate_type: 'Day Rate', rate_amount: '€350/day' }];

-- 34
CREATE job_posting SET title = 'SFX Supervisor — Practical Effects', description = 'Horror feature using exclusively practical effects. Blood rigs, animatronics, atmospheric effects. No CGI. 4-week shoot.', location = 'London, UK', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'SFX Supervisor', description: 'Design and execute all practical effects', rate_type: 'Weekly', rate_amount: '€3,500/week' }];

-- 35
CREATE job_posting SET title = 'Assistant Editor — Documentary', description = 'Join the edit room for a 3-part wildlife documentary. Avid Media Composer. Syncing, assembling selects, and managing bins. 8-week contract.', location = 'Bristol, UK', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Assistant Editor', description: 'Avid workflow, syncing, bin management', rate_type: 'Weekly', rate_amount: '€1,200/week' }];

-- 36
CREATE job_posting SET title = 'Transport Captain — Feature Film', description = 'Large-scale production needs transport department head to manage fleet of unit vehicles, talent transport, and equipment moves across 15 locations.', location = 'Warsaw, Poland', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Transport Captain', description: 'Manage all transport logistics', rate_type: 'Weekly', rate_amount: '€1,500/week' }, { title: 'Driver — Unit', description: 'Crew and equipment transport, clean license', rate_type: 'Day Rate', rate_amount: '€180/day' }];

-- 37
CREATE job_posting SET title = 'Dialogue Coach — International Cast', description = 'Feature film with cast from 4 different countries needs a dialogue coach to ensure consistent accent and delivery. On set throughout principal photography.', location = 'Dublin, Ireland', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Dialogue Coach', description: 'Accent consistency and performance support', rate_type: 'Day Rate', rate_amount: '€400/day' }];

-- 38
CREATE job_posting SET title = 'Catering — Film Crew (60 pax)', description = 'Professional film catering company or chef needed for 25-day shoot. Crew of 60. Breakfast, lunch, and afternoon snack. Dietary accommodations required.', location = 'Cologne, Germany', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 7d, roles = [{ title: 'Head Chef / Caterer', description: 'Full crew catering, 60 people, 25 days', rate_type: 'Day Rate', rate_amount: '€1,200/day' }];

-- 39
CREATE job_posting SET title = 'Underwater Camera Operator', description = 'Ocean thriller requires experienced underwater cinematographer. Open water and tank work. Own housing for RED or Alexa Mini required.', location = 'Marseille, France', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Underwater Camera Operator', description: 'Open water and tank cinematography', rate_type: 'Day Rate', rate_amount: '€900/day' }];

-- 40
CREATE job_posting SET title = 'Graphic Designer — Film Marketing', description = 'Creating key art, social media assets, and festival materials for three independent films. Must understand film industry aesthetics and deliverable specs.', location = 'Remote', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Graphic Designer', description: 'Key art, posters, social assets for 3 films', rate_type: 'Flat', rate_amount: '€4,000 total' }];

-- 41
CREATE job_posting SET title = 'Armourer — War Film', description = 'Period war film set in WWII requires licensed armourer for handling replica and deactivated weapons. Must hold valid firearms license for film work.', location = 'Kent, UK', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Armourer', description: 'Weapons handling, safety, and continuity', rate_type: 'Day Rate', rate_amount: '€550/day' }];

-- 42
CREATE job_posting SET title = 'Runners — Various Productions', description = 'Production company seeking reliable runners for multiple upcoming projects. Ideal entry point for anyone wanting to break into the industry.', location = 'London, UK', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Set Runner', description: 'General set duties, must be proactive and reliable', rate_type: 'Day Rate', rate_amount: '€130/day' }, { title: 'Office Runner', description: 'Production office support, errands, deliveries', rate_type: 'Day Rate', rate_amount: '€120/day' }];

-- 43
CREATE job_posting SET title = "Puppeteer — Children's TV Show", description = "Established children's show seeking new puppeteers for Season 5. Rod and hand puppet experience required. Auditions in person only.", location = 'Munich, Germany', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Puppeteer', description: 'Rod and hand puppetry, voice work', rate_type: 'Day Rate', rate_amount: '€350/day' }];

-- 44
CREATE job_posting SET title = 'Colourist — Feature Film', description = 'Award-winning cinematographer seeks a colourist for their latest feature. Shot on 35mm and scanned at 4K. Baselight preferred.', location = 'London, UK', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Colourist', description: '4K feature grade, Baselight, 35mm scan', rate_type: 'Flat', rate_amount: '€10,000' }];

-- 45
CREATE job_posting SET title = 'Production Coordinator — Corporate Films', description = 'Ongoing work coordinating corporate video productions for tech companies. 3-5 shoots per month. Must be highly organized with strong communication skills.', location = 'Berlin, Germany', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Production Coordinator', description: 'Schedule, book crew, manage logistics for corporate shoots', rate_type: 'Monthly', rate_amount: '€3,500/month' }];

-- 46
CREATE job_posting SET title = 'Stills Photographer — Film Set', description = 'Unit stills photographer needed for behind-the-scenes and publicity shots on a high-profile feature film. Portfolio required.', location = 'Barcelona, Spain', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Unit Stills Photographer', description: 'BTS and publicity photography', rate_type: 'Day Rate', rate_amount: '€350/day' }];

-- 47
CREATE job_posting SET title = 'EPK Crew — Behind the Scenes', description = 'Electronic Press Kit crew needed for a major studio production. Interviews, BTS footage, and social media content. 4-week assignment.', location = 'Paris, France', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 7d, roles = [{ title: 'EPK Director', description: 'Direct and produce all BTS content', rate_type: 'Weekly', rate_amount: '€2,000/week' }, { title: 'EPK Camera Operator', description: 'Shoot interviews and BTS footage', rate_type: 'Day Rate', rate_amount: '€350/day' }];

-- 48
CREATE job_posting SET title = 'Art Director — Music Video', description = 'Concept-heavy music video for a major label artist. Surrealist aesthetic, bold colour palette. 2 prep days + 1 shoot day.', location = 'Berlin, Germany', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 7d, roles = [{ title: 'Art Director', description: 'Conceptual design, set builds, colour direction', rate_type: 'Flat', rate_amount: '€2,500' }];

-- 49
CREATE job_posting SET title = 'Legal Counsel — Film Production', description = 'Independent production company needs entertainment lawyer for contract review, chain of title, and distribution agreements. Project-based work.', location = 'Remote', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Entertainment Lawyer', description: 'Contracts, chain of title, distribution deals', rate_type: 'Hourly', rate_amount: '€200/hour' }];

-- 50
CREATE job_posting SET title = 'Carpenter — Set Construction', description = 'Experienced set carpenter for studio build. Reading technical drawings, timber framing, and scenic finishing. 3-week build period.', location = 'Babelsberg, Germany', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Set Carpenter', description: 'Studio set construction from technical drawings', rate_type: 'Day Rate', rate_amount: '€280/day' }];

-- 51
CREATE job_posting SET title = 'Movement Director — Theatre-to-Film Adaptation', description = 'Stage play being adapted for screen. Movement director needed to translate theatrical blocking into cinematic choreography. Rehearsal intensive.', location = 'Manchester, UK', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Movement Director', description: 'Physical performance coaching and blocking', rate_type: 'Day Rate', rate_amount: '€450/day' }];

-- 52
CREATE job_posting SET title = 'Playback Operator — Live Concert Film', description = 'Multi-camera live concert recording. Playback operator to manage video village, instant replay, and live switching for director reference.', location = 'Zurich, Switzerland', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 7d, roles = [{ title: 'Playback Operator', description: 'Video village management, live playback', rate_type: 'Day Rate', rate_amount: '€400/day' }];

-- 53
CREATE job_posting SET title = 'Subtitler & Translator — Film Festival', description = 'Five feature films need subtitling from German to English and French for festival submissions. SRT format. Quick turnaround.', location = 'Remote', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Subtitler — German to English', description: '3 features, SRT format', rate_type: 'Flat', rate_amount: '€600 per film' }, { title: 'Subtitler — German to French', description: '2 features, SRT format', rate_type: 'Flat', rate_amount: '€600 per film' }];

-- 54
CREATE job_posting SET title = 'Animal Wrangler — Period Drama', description = 'Period drama featuring horses, dogs, and chickens. Licensed animal handler with film set experience required. Animal welfare documentation mandatory.', location = 'Yorkshire, UK', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Animal Wrangler', description: 'Handle and coordinate animals on set', rate_type: 'Day Rate', rate_amount: '€450/day' }];

-- 55
CREATE job_posting SET title = 'Re-Recording Mixer — Feature Film', description = 'Dolby Atmos mix for a 100-minute drama. Working from a fully prepped session. 2-week mix at our facility in Munich.', location = 'Munich, Germany', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Re-Recording Mixer', description: 'Dolby Atmos final mix, dialogue and music', rate_type: 'Flat', rate_amount: '€8,000' }];

-- 56
CREATE job_posting SET title = 'Second Unit Director — Adventure Film', description = 'Action-adventure feature needs a second unit to handle landscapes, inserts, and some action sequences independently. 2-week schedule.', location = 'Reykjavik, Iceland', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: '2nd Unit Director', description: 'Direct all second unit photography', rate_type: 'Weekly', rate_amount: '€3,500/week' }, { title: '2nd Unit DP', description: 'Match main unit look and feel', rate_type: 'Day Rate', rate_amount: '€600/day' }];

-- 57
CREATE job_posting SET title = 'Dialect Coach — Irish Accent', description = 'American lead actor needs coaching for authentic Dublin accent. Pre-production coaching via video calls + on-set support during key scenes.', location = 'Dublin, Ireland', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Dialect Coach', description: 'Irish accent coaching for lead performer', rate_type: 'Day Rate', rate_amount: '€400/day' }];

-- 58
CREATE job_posting SET title = 'Social Media Manager — Film Release', description = 'Managing social channels during theatrical release campaign. Content creation, community management, and analytics. 3-month contract.', location = 'Remote', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Social Media Manager', description: 'Campaign management across Instagram, TikTok, X', rate_type: 'Monthly', rate_amount: '€2,500/month' }];

-- 59
CREATE job_posting SET title = 'Rigger — Studio Shoot', description = 'Green screen studio shoot needs experienced riggers for overhead lighting grids, backdrop systems, and safety equipment. LOLER certified.', location = 'London, UK', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 7d, roles = [{ title: 'Rigger', description: 'Overhead rigging, LOLER certified', rate_type: 'Day Rate', rate_amount: '€350/day' }];

-- 60
CREATE job_posting SET title = 'Voice-Over Artists — Animation Series', description = 'New animated comedy series casting multiple voice roles. Range of characters from villains to comic sidekicks. Home studio setup required.', location = 'Remote', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Voice Actor — Lead Hero', description: 'Male, energetic, comedic timing', rate_type: 'Flat', rate_amount: '€500 per episode' }, { title: 'Voice Actor — Villain', description: 'Deep, menacing, theatrical', rate_type: 'Flat', rate_amount: '€400 per episode' }, { title: 'Voice Actor — Sidekick', description: 'High energy, improvisational skills', rate_type: 'Flat', rate_amount: '€400 per episode' }];

-- 61
CREATE job_posting SET title = 'Visual Effects Supervisor — On Set', description = 'VFX supe needed on set to oversee plate acquisition, track markers, HDRI capture, and advise director on feasibility. 6-week shoot.', location = 'Helsinki, Finland', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'VFX Supervisor', description: 'On-set supervision, data acquisition', rate_type: 'Day Rate', rate_amount: '€700/day' }];

-- 62
CREATE job_posting SET title = 'Script Reader — Production Company', description = 'Production company seeks freelance script readers for coverage. 3-5 scripts per week. Must write clear, concise coverage with market awareness.', location = 'Remote', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Script Reader', description: 'Script coverage and recommendations', rate_type: 'Flat', rate_amount: '€80 per script' }];

-- 63
CREATE job_posting SET title = '2nd Assistant Director — Feature Film', description = 'Supporting the 1st AD on a complex multi-location shoot. Call sheet preparation, extras coordination, and set logistics. Must be detail-oriented.', location = 'Lyon, France', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: '2nd Assistant Director', description: 'Call sheets, extras wrangling, set support', rate_type: 'Day Rate', rate_amount: '€250/day' }];

-- 64
CREATE job_posting SET title = 'Foley Artist — Feature Film', description = 'Full Foley pass for a 95-minute thriller. Footsteps, cloth, props, and specifics. Our studio, your creativity. 10-day booking.', location = 'Berlin, Germany', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Foley Artist', description: 'Full Foley coverage, 10-day session', rate_type: 'Flat', rate_amount: '€5,000' }];

-- 65
CREATE job_posting SET title = 'Marine Coordinator — Coastal Shoot', description = 'Thriller set on a fishing trawler. Need marine coordinator for boat operations, water safety, and maritime logistics. 3 weeks at sea.', location = 'Bergen, Norway', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Marine Coordinator', description: 'Vessel operations and water safety', rate_type: 'Day Rate', rate_amount: '€600/day' }];

-- 66
CREATE job_posting SET title = 'Casting Associate — Feature Film', description = 'Assist casting director with session scheduling, self-tape management, and actor shortlists. Must know Casting Networks and Spotlight.', location = 'London, UK', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Casting Associate', description: 'Session management, shortlists, self-tape review', rate_type: 'Weekly', rate_amount: '€800/week' }];

-- 67
CREATE job_posting SET title = 'Lighting Technician — TV Studio', description = 'Permanent TV studio needs lighting technicians for variety shows and talk shows. Rotating schedule, must be flexible with hours.', location = 'Cologne, Germany', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Lighting Technician', description: 'Studio lighting operation, moving heads, dimmers', rate_type: 'Day Rate', rate_amount: '€250/day' }];

-- 68
CREATE job_posting SET title = 'Publicist — Film Festival Circuit', description = 'Indie feature needs a publicist to manage festival premiere, press screenings, and journalist outreach. Cannes and Venice experience preferred.', location = 'Remote', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Film Publicist', description: 'Festival PR, press outreach, screening coordination', rate_type: 'Flat', rate_amount: '€8,000' }];

-- 69
CREATE job_posting SET title = 'Scenic Painter — Film Studio', description = 'Experienced scenic painter for backdrops, faux finishes, and aged surfaces. Knowledge of theatrical and film painting techniques. 4-week build.', location = 'Babelsberg, Germany', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Scenic Painter', description: 'Backdrops, aging, faux finishing', rate_type: 'Day Rate', rate_amount: '€280/day' }];

-- 70
CREATE job_posting SET title = 'Production Accountant — TV Series', description = 'Managing the financial side of a 10-episode series. Weekly cost reports, petty cash, purchase orders, and wrap accounting. Must know Movie Magic.', location = 'London, UK', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Production Accountant', description: 'Full production accounting, Movie Magic Budgeting', rate_type: 'Weekly', rate_amount: '€2,000/week' }, { title: 'Accounts Assistant', description: 'Purchase orders, petty cash, payroll support', rate_type: 'Weekly', rate_amount: '€900/week' }];

-- 71
CREATE job_posting SET title = 'Crane Operator — Architectural Film', description = 'Documentary about Brutalist architecture. Need crane operator with experience shooting buildings. Technocrane or similar. 5 shoot days.', location = 'Belgrade, Serbia', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Crane Operator', description: 'Technocrane operation for architectural shots', rate_type: 'Day Rate', rate_amount: '€600/day' }];

-- 72
CREATE job_posting SET title = 'Stand-In — Feature Film', description = 'Looking for stand-ins to match principal cast for lighting and blocking. Must be available for full 5-week shoot. Heights and builds specified per role.', location = 'Dublin, Ireland', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 7d, roles = [{ title: 'Stand-In — Male Lead', description: '180cm, medium build, light complexion', rate_type: 'Day Rate', rate_amount: '€150/day' }, { title: 'Stand-In — Female Lead', description: '170cm, slim build, dark hair', rate_type: 'Day Rate', rate_amount: '€150/day' }];

-- 73
CREATE job_posting SET title = 'Pyrotechnics Supervisor', description = 'Action sequence involving controlled explosions and fire effects. Licensed pyrotechnician with insurance and all required permits.', location = 'Sofia, Bulgaria', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Pyrotechnics Supervisor', description: 'Controlled explosions and fire effects, fully licensed', rate_type: 'Day Rate', rate_amount: '€800/day' }];

-- 74
CREATE job_posting SET title = 'Researcher — True Crime Documentary', description = 'In-depth research for a 4-part true crime documentary series. Archival research, witness outreach, and fact verification. Remote with occasional travel.', location = 'Remote', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Researcher', description: 'Archival research, interviews, fact-checking', rate_type: 'Monthly', rate_amount: '€3,000/month' }];

-- 75
CREATE job_posting SET title = 'Boom Operator — Feature Film', description = 'Drama shooting on location in rural Tuscany. Boom operator with experience in exterior dialogue recording. 4-week shoot. Accommodation provided.', location = 'Tuscany, Italy', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Boom Operator', description: 'Location dialogue recording, exteriors', rate_type: 'Day Rate', rate_amount: '€280/day' }];

-- 76
CREATE job_posting SET title = 'Title Designer — Opening Credits', description = 'Creating a bespoke opening title sequence for a noir-inspired thriller. Motion graphics and typography. References: Saul Bass, Art of the Title.', location = 'Remote', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Title Designer', description: 'Main title sequence design and animation', rate_type: 'Flat', rate_amount: '€5,000' }];

-- 77
CREATE job_posting SET title = 'Security — Night Shoots', description = 'Film production needs security personnel for overnight exterior shoots in urban locations. Licensed security guards only. Multiple dates.', location = 'Hamburg, Germany', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 7d, roles = [{ title: 'Security Guard', description: 'Overnight set security, licensed and insured', rate_type: 'Hourly', rate_amount: '€25/hour' }];

-- 78
CREATE job_posting SET title = 'Casting — Voice Actors for Video Game', description = 'AAA video game title casting multiple characters. Motion capture and voice performance. Must be available for sessions in London over 3 months.', location = 'London, UK', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Voice Actor — Protagonist', description: 'Male, 30-40, dramatic range, mocap required', rate_type: 'Day Rate', rate_amount: '€800/day' }, { title: 'Voice Actor — Antagonist', description: 'Female, 35-50, commanding presence, mocap required', rate_type: 'Day Rate', rate_amount: '€800/day' }];

-- 79
CREATE job_posting SET title = 'Storyboard Artist — Animated Feature', description = 'Hand-drawn animated feature needs storyboard artists to translate script into visual sequences. Strong drawing skills and storytelling sense essential.', location = 'Remote', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Storyboard Artist', description: 'Full sequence boarding, digital or traditional', rate_type: 'Weekly', rate_amount: '€1,500/week' }];

-- 80
CREATE job_posting SET title = 'Health & Safety Officer — Film Set', description = 'H&S officer needed for a stunt-heavy action film. Risk assessments, safety briefings, and emergency planning. NEBOSH certified.', location = 'Prague, Czech Republic', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Health & Safety Officer', description: 'Risk assessment, safety supervision, NEBOSH certified', rate_type: 'Day Rate', rate_amount: '€350/day' }];

-- 81
CREATE job_posting SET title = 'Drone Pilot — Real Estate Promos', description = 'Ongoing work filming luxury properties from the air for a real estate agency. 2-3 properties per week across the Riviera.', location = 'Nice, France', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Drone Pilot', description: 'Aerial property photography and video', rate_type: 'Flat', rate_amount: '€300 per property' }];

-- 82
CREATE job_posting SET title = 'Steadicam Operator — TV Drama', description = 'Hospital drama uses predominantly Steadicam for its signature look. Season 4 needs an operator for 12-week block. Must own Tiffen or GPI rig.', location = 'Cologne, Germany', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Steadicam Operator', description: 'Primary camera operation, 12-week block', rate_type: 'Day Rate', rate_amount: '€650/day' }];

-- 83
CREATE job_posting SET title = 'Music Supervisor — Indie Film', description = 'Curating the soundtrack for a Berlin-set drama. Mix of licensed tracks and original score. Budget for licensing: €50K. Taste and negotiation skills needed.', location = 'Berlin, Germany', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Music Supervisor', description: 'Curate, license, and supervise all music', rate_type: 'Flat', rate_amount: '€7,000' }];

-- 84
CREATE job_posting SET title = 'Electrician — Film Lighting', description = 'Qualified electrician for temporary power installations on film sets. Three-phase distribution, generator operation, and lighting power.', location = 'London, UK', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Electrician', description: 'Temporary power distribution, generators', rate_type: 'Day Rate', rate_amount: '€350/day' }];

-- 85
CREATE job_posting SET title = 'Completion Guarantor Liaison', description = 'Production company needs someone to manage completion bond requirements, daily progress reporting, and cost monitoring for financiers.', location = 'Remote', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Bond Liaison', description: 'Completion bond reporting and compliance', rate_type: 'Weekly', rate_amount: '€1,500/week' }];

-- 86
CREATE job_posting SET title = 'Choreographer — Musical Film', description = 'Original movie musical needs a choreographer for 8 dance numbers. Broadway or West End experience preferred. Pre-production workshops included.', location = 'London, UK', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Choreographer', description: 'Design and teach 8 musical numbers', rate_type: 'Flat', rate_amount: '€25,000' }, { title: 'Dance Captain', description: 'Maintain choreography on set, rehearse performers', rate_type: 'Weekly', rate_amount: '€1,200/week' }];

-- 87
CREATE job_posting SET title = 'Medic — Remote Location Shoot', description = 'First aid qualified medic for a 3-week shoot in a remote mountain location. Must hold current first aid at work certificate and be comfortable outdoors.', location = 'Innsbruck, Austria', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 7d, roles = [{ title: 'On-Set Medic', description: 'First aid, remote location, outdoor work', rate_type: 'Day Rate', rate_amount: '€300/day' }];

-- 88
CREATE job_posting SET title = "Showrunner's Assistant — TV Series", description = "Assisting the showrunner on a prestige drama series. Script management, scheduling, note-taking, and general support. Incredibly demanding but career-defining.", location = 'Berlin, Germany', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: "Showrunner's Assistant", description: 'Executive support, script management, scheduling', rate_type: 'Weekly', rate_amount: '€800/week' }];

-- 89
CREATE job_posting SET title = 'VR/360 Camera Operator', description = 'Immersive documentary project shooting in 360 VR. Experience with Insta360 Pro or similar required. Stitching and post knowledge a plus.', location = 'Amsterdam, Netherlands', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: '360 Camera Operator', description: '360 VR cinematography, stitching experience', rate_type: 'Day Rate', rate_amount: '€500/day' }];

-- 90
CREATE job_posting SET title = 'Set Photographer — Food Film', description = 'Culinary documentary needs a photographer who specializes in food photography. Capturing cooking processes and finished dishes in natural light.', location = 'Bologna, Italy', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 7d, roles = [{ title: 'Food Photographer', description: 'Food and cooking process photography', rate_type: 'Day Rate', rate_amount: '€400/day' }];

-- 91
CREATE job_posting SET title = 'Clearance Coordinator — Feature Film', description = 'Review all visual elements for potential IP issues. Clear brands, artwork, music, and signage appearing on screen. Legal background helpful.', location = 'Remote', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Clearance Coordinator', description: 'Visual and music clearances, IP review', rate_type: 'Weekly', rate_amount: '€1,000/week' }];

-- 92
CREATE job_posting SET title = 'Colourist Assistant — Commercial Block', description = 'Support senior colourist during busy commercial season. Conform, renders, and deliverables in DaVinci Resolve. Fast learner, detail-oriented.', location = 'Hamburg, Germany', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'Colourist Assistant', description: 'Conform, renders, DaVinci Resolve', rate_type: 'Day Rate', rate_amount: '€200/day' }];

-- 93
CREATE job_posting SET title = 'Extras Casting — Festival Scene', description = 'Period drama needs 200+ background performers for a festival scene. All ages, diverse appearances. Costumes provided. 2-day shoot.', location = 'Krakow, Poland', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 7d, roles = [{ title: 'Background Performer', description: 'All ages and appearances welcome, 2-day commitment', rate_type: 'Day Rate', rate_amount: '€80/day' }];

-- 94
CREATE job_posting SET title = 'Aerial Coordinator — Helicopter Shots', description = 'Feature film requires aerial sequences shot from helicopter. Coordinate flight plans, camera mounts, and safety. CAA approved.', location = 'Edinburgh, UK', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'Aerial Coordinator', description: 'Helicopter cinematography coordination', rate_type: 'Day Rate', rate_amount: '€1,000/day' }];

-- 95
CREATE job_posting SET title = 'ADR Supervisor — Foreign Language Film', description = 'German-language film needs ADR supervisor for pickup dialogue and performance matching. Must understand German. Pro Tools required.', location = 'Berlin, Germany', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'ADR Supervisor', description: 'German-language ADR recording and supervision', rate_type: 'Flat', rate_amount: '€4,000' }];

-- 96
CREATE job_posting SET title = 'Construction Manager — Studio Build', description = 'Overseeing construction of multiple standing sets for a long-running TV series. Team management, budgets, and scheduling. 6-month contract.', location = 'Babelsberg, Germany', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Construction Manager', description: 'Manage full build team, budgets, and schedules', rate_type: 'Weekly', rate_amount: '€2,500/week' }];

-- 97
CREATE job_posting SET title = 'BTS Content Creator — Social Media', description = 'Create engaging vertical video content from film sets for TikTok and Instagram Reels. Must be discreet and understand set etiquette.', location = 'Various — Germany', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 21d, roles = [{ title: 'BTS Content Creator', description: 'Short-form video for TikTok and Reels', rate_type: 'Day Rate', rate_amount: '€300/day' }];

-- 98
CREATE job_posting SET title = 'Negative Cutter — Archive Restoration', description = 'Film archive restoration project needs experienced negative cutter for 16mm and 35mm. Handling fragile material with extreme care. Gloves-on work.', location = 'Munich, Germany', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Negative Cutter', description: '16mm and 35mm negative handling and cutting', rate_type: 'Day Rate', rate_amount: '€350/day' }];

-- 99
CREATE job_posting SET title = 'COVID Safety Supervisor', description = 'Production company maintaining safety protocols seeks supervisor for testing coordination, ventilation assessment, and on-set compliance.', location = 'London, UK', posted_by = $seceda, applications_enabled = true, status = 'open', expires_at = time::now() + 14d, roles = [{ title: 'COVID Safety Supervisor', description: 'Testing, ventilation, and protocol enforcement', rate_type: 'Day Rate', rate_amount: '€300/day' }];

-- 100
CREATE job_posting SET title = 'Showreel Editor — Actors', description = 'Experienced editor offering showreel editing for actors. Quick turnaround, competitive rates. Understand casting director expectations and pacing.', location = 'Remote', posted_by = $chris, applications_enabled = true, status = 'open', expires_at = time::now() + 30d, roles = [{ title: 'Showreel Editor', description: 'Edit actor showreels from existing footage', rate_type: 'Flat', rate_amount: '€250 per reel' }];
