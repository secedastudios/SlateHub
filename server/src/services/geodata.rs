/// Geographic city-to-region mapping for embedding text enrichment.
/// Maps city name triggers to their full geographic context (state/province, country, region).
/// Covers major filming locations, national capitals, US state capitals, Canadian provincial capitals,
/// and significant cities worldwide.

/// Returns expanded location text with full geographic context.
/// "Berlin" → "berlin, germany, europe"
/// "Los Angeles, CA" → "los angeles, ca, california, united states, north america"
pub fn expand_location(location: &str) -> String {
    let loc_lower = location.to_lowercase();

    let mut result = loc_lower.clone();
    for (triggers, expansion) in CITIES {
        if triggers.iter().any(|t| loc_lower.contains(t)) {
            for part in expansion.split(", ") {
                if !result.contains(part) {
                    result.push_str(", ");
                    result.push_str(part);
                }
            }
        }
    }

    result
}

const CITIES: &[(&[&str], &str)] = &[
    // =========================================================================
    // UNITED STATES — State capitals, major cities, and film production hubs
    // =========================================================================

    // Alabama
    (&["montgomery"], "montgomery, alabama, united states, north america"),
    (&["birmingham, al", "birmingham, alabama"], "birmingham, alabama, united states, north america"),

    // Alaska
    (&["juneau"], "juneau, alaska, united states, north america"),
    (&["anchorage"], "anchorage, alaska, united states, north america"),

    // Arizona
    (&["phoenix"], "phoenix, arizona, united states, north america"),
    (&["scottsdale"], "scottsdale, arizona, united states, north america"),
    (&["tucson"], "tucson, arizona, united states, north america"),

    // Arkansas
    (&["little rock"], "little rock, arkansas, united states, north america"),

    // California — major film hub
    (&["los angeles", "l.a."], "los angeles, california, united states, north america, hollywood"),
    (&["hollywood"], "hollywood, los angeles, california, united states, north america"),
    (&["burbank"], "burbank, los angeles, california, united states, north america"),
    (&["culver city"], "culver city, los angeles, california, united states, north america"),
    (&["santa monica"], "santa monica, los angeles, california, united states, north america"),
    (&["san francisco"], "san francisco, california, united states, north america"),
    (&["san diego"], "san diego, california, united states, north america"),
    (&["sacramento"], "sacramento, california, united states, north america"),
    (&["oakland"], "oakland, california, united states, north america"),
    (&["san jose"], "san jose, california, united states, north america"),
    (&["long beach"], "long beach, california, united states, north america"),
    (&["santa barbara"], "santa barbara, california, united states, north america"),
    (&["palm springs"], "palm springs, california, united states, north america"),

    // Colorado
    (&["denver"], "denver, colorado, united states, north america"),
    (&["boulder"], "boulder, colorado, united states, north america"),

    // Connecticut
    (&["hartford"], "hartford, connecticut, united states, north america"),
    (&["stamford"], "stamford, connecticut, united states, north america"),
    (&["new haven"], "new haven, connecticut, united states, north america"),

    // Delaware
    (&["dover, de", "dover, delaware"], "dover, delaware, united states, north america"),
    (&["wilmington, de", "wilmington, delaware"], "wilmington, delaware, united states, north america"),

    // Florida
    (&["tallahassee"], "tallahassee, florida, united states, north america"),
    (&["miami"], "miami, florida, united states, north america"),
    (&["orlando"], "orlando, florida, united states, north america"),
    (&["tampa"], "tampa, florida, united states, north america"),
    (&["jacksonville"], "jacksonville, florida, united states, north america"),
    (&["fort lauderdale"], "fort lauderdale, florida, united states, north america"),

    // Georgia — major film hub
    (&["atlanta"], "atlanta, georgia, united states, north america"),
    (&["savannah"], "savannah, georgia, united states, north america"),

    // Hawaii
    (&["honolulu"], "honolulu, hawaii, united states, north america"),

    // Idaho
    (&["boise"], "boise, idaho, united states, north america"),

    // Illinois
    (&["chicago"], "chicago, illinois, united states, north america"),
    (&["springfield, il", "springfield, illinois"], "springfield, illinois, united states, north america"),

    // Indiana
    (&["indianapolis"], "indianapolis, indiana, united states, north america"),

    // Iowa
    (&["des moines"], "des moines, iowa, united states, north america"),

    // Kansas
    (&["topeka"], "topeka, kansas, united states, north america"),
    (&["wichita"], "wichita, kansas, united states, north america"),

    // Kentucky
    (&["frankfort, ky", "frankfort, kentucky"], "frankfort, kentucky, united states, north america"),
    (&["louisville"], "louisville, kentucky, united states, north america"),
    (&["lexington, ky", "lexington, kentucky"], "lexington, kentucky, united states, north america"),

    // Louisiana — major film hub
    (&["baton rouge"], "baton rouge, louisiana, united states, north america"),
    (&["new orleans"], "new orleans, louisiana, united states, north america"),
    (&["shreveport"], "shreveport, louisiana, united states, north america"),

    // Maine
    (&["augusta, me", "augusta, maine"], "augusta, maine, united states, north america"),
    (&["portland, me", "portland, maine"], "portland, maine, united states, north america"),

    // Maryland
    (&["annapolis"], "annapolis, maryland, united states, north america"),
    (&["baltimore"], "baltimore, maryland, united states, north america"),

    // Massachusetts
    (&["boston"], "boston, massachusetts, united states, north america"),
    (&["cambridge, ma", "cambridge, mass"], "cambridge, massachusetts, united states, north america"),

    // Michigan
    (&["lansing"], "lansing, michigan, united states, north america"),
    (&["detroit"], "detroit, michigan, united states, north america"),
    (&["grand rapids"], "grand rapids, michigan, united states, north america"),

    // Minnesota
    (&["st. paul", "saint paul, mn"], "st. paul, minnesota, united states, north america"),
    (&["minneapolis"], "minneapolis, minnesota, united states, north america"),

    // Mississippi
    (&["jackson, ms", "jackson, mississippi"], "jackson, mississippi, united states, north america"),

    // Missouri
    (&["jefferson city"], "jefferson city, missouri, united states, north america"),
    (&["st. louis", "saint louis"], "st. louis, missouri, united states, north america"),
    (&["kansas city"], "kansas city, missouri, united states, north america"),

    // Montana
    (&["helena, mt", "helena, montana"], "helena, montana, united states, north america"),

    // Nebraska
    (&["lincoln, ne", "lincoln, nebraska"], "lincoln, nebraska, united states, north america"),
    (&["omaha"], "omaha, nebraska, united states, north america"),

    // Nevada
    (&["carson city"], "carson city, nevada, united states, north america"),
    (&["las vegas"], "las vegas, nevada, united states, north america"),

    // New Hampshire
    (&["concord, nh", "concord, new hampshire"], "concord, new hampshire, united states, north america"),

    // New Jersey
    (&["trenton"], "trenton, new jersey, united states, north america"),
    (&["newark, nj", "newark, new jersey"], "newark, new jersey, united states, north america"),
    (&["jersey city"], "jersey city, new jersey, united states, north america"),

    // New Mexico — film hub
    (&["santa fe"], "santa fe, new mexico, united states, north america"),
    (&["albuquerque"], "albuquerque, new mexico, united states, north america"),

    // New York — major film hub
    (&["new york", "nyc", "manhattan", "brooklyn"], "new york city, new york, united states, north america"),
    (&["albany, ny", "albany, new york"], "albany, new york, united states, north america"),
    (&["queens"], "queens, new york city, new york, united states, north america"),
    (&["long island"], "long island, new york, united states, north america"),

    // North Carolina — major film hub
    (&["raleigh"], "raleigh, north carolina, united states, north america"),
    (&["charlotte"], "charlotte, north carolina, united states, north america"),
    (&["wilmington, nc", "wilmington, north carolina"], "wilmington, north carolina, united states, north america"),
    (&["durham, nc", "durham, north carolina"], "durham, north carolina, united states, north america"),

    // North Dakota
    (&["bismarck"], "bismarck, north dakota, united states, north america"),

    // Ohio
    (&["columbus, oh", "columbus, ohio"], "columbus, ohio, united states, north america"),
    (&["cleveland"], "cleveland, ohio, united states, north america"),
    (&["cincinnati"], "cincinnati, ohio, united states, north america"),

    // Oklahoma
    (&["oklahoma city"], "oklahoma city, oklahoma, united states, north america"),

    // Oregon
    (&["salem, or", "salem, oregon"], "salem, oregon, united states, north america"),
    (&["portland, or", "portland, oregon"], "portland, oregon, united states, north america"),

    // Pennsylvania
    (&["harrisburg"], "harrisburg, pennsylvania, united states, north america"),
    (&["philadelphia"], "philadelphia, pennsylvania, united states, north america"),
    (&["pittsburgh"], "pittsburgh, pennsylvania, united states, north america"),

    // Rhode Island
    (&["providence"], "providence, rhode island, united states, north america"),

    // South Carolina
    (&["columbia, sc", "columbia, south carolina"], "columbia, south carolina, united states, north america"),
    (&["charleston, sc", "charleston, south carolina"], "charleston, south carolina, united states, north america"),

    // South Dakota
    (&["pierre"], "pierre, south dakota, united states, north america"),

    // Tennessee
    (&["nashville"], "nashville, tennessee, united states, north america"),
    (&["memphis"], "memphis, tennessee, united states, north america"),

    // Texas — film hub
    (&["austin"], "austin, texas, united states, north america"),
    (&["dallas"], "dallas, texas, united states, north america"),
    (&["houston"], "houston, texas, united states, north america"),
    (&["san antonio"], "san antonio, texas, united states, north america"),

    // Utah
    (&["salt lake city"], "salt lake city, utah, united states, north america"),
    (&["park city"], "park city, utah, united states, north america, sundance"),

    // Vermont
    (&["montpelier, vt", "montpelier, vermont"], "montpelier, vermont, united states, north america"),
    (&["burlington, vt", "burlington, vermont"], "burlington, vermont, united states, north america"),

    // Virginia
    (&["richmond, va", "richmond, virginia"], "richmond, virginia, united states, north america"),
    (&["norfolk"], "norfolk, virginia, united states, north america"),

    // Washington
    (&["olympia, wa", "olympia, washington"], "olympia, washington, united states, north america"),
    (&["seattle"], "seattle, washington, united states, north america"),

    // West Virginia
    (&["charleston, wv", "charleston, west virginia"], "charleston, west virginia, united states, north america"),

    // Wisconsin
    (&["madison, wi", "madison, wisconsin"], "madison, wisconsin, united states, north america"),
    (&["milwaukee"], "milwaukee, wisconsin, united states, north america"),

    // Wyoming
    (&["cheyenne"], "cheyenne, wyoming, united states, north america"),

    // US State abbreviations → full names (for patterns like "City, CA")
    (&[", ca", ", california"], "california, united states"),
    (&[", ny", ", new york"], "new york, united states"),
    (&[", tx", ", texas"], "texas, united states"),
    (&[", ga", ", georgia"], "georgia, united states"),
    (&[", il", ", illinois"], "illinois, united states"),
    (&[", fl", ", florida"], "florida, united states"),
    (&[", wa ", ", washington"], "washington state, united states"),
    (&[", or ", ", oregon"], "oregon, united states"),
    (&[", co", ", colorado"], "colorado, united states"),
    (&[", ma", ", massachusetts"], "massachusetts, united states"),
    (&[", tn", ", tennessee"], "tennessee, united states"),
    (&[", nc", ", north carolina"], "north carolina, united states"),
    (&[", pa", ", pennsylvania"], "pennsylvania, united states"),
    (&[", nj", ", new jersey"], "new jersey, united states"),
    (&[", oh", ", ohio"], "ohio, united states"),
    (&[", mi", ", michigan"], "michigan, united states"),
    (&[", mn", ", minnesota"], "minnesota, united states"),
    (&[", az", ", arizona"], "arizona, united states"),
    (&[", nv", ", nevada"], "nevada, united states"),
    (&[", ut", ", utah"], "utah, united states"),
    (&[", hi", ", hawaii"], "hawaii, united states"),
    (&[", nm", ", new mexico"], "new mexico, united states"),
    (&[", la", ", louisiana"], "louisiana, united states"),
    (&[", md", ", maryland"], "maryland, united states"),
    (&[", va", ", virginia"], "virginia, united states"),
    (&[", sc", ", south carolina"], "south carolina, united states"),
    (&[", ct", ", connecticut"], "connecticut, united states"),

    // =========================================================================
    // CANADA — Provincial capitals and major film cities
    // =========================================================================
    (&["vancouver"], "vancouver, british columbia, canada, north america"),
    (&["victoria, bc", "victoria, british columbia"], "victoria, british columbia, canada, north america"),
    (&["toronto"], "toronto, ontario, canada, north america"),
    (&["ottawa"], "ottawa, ontario, canada, north america"),
    (&["montreal", "montréal"], "montreal, quebec, canada, north america"),
    (&["quebec city"], "quebec city, quebec, canada, north america"),
    (&["calgary"], "calgary, alberta, canada, north america"),
    (&["edmonton"], "edmonton, alberta, canada, north america"),
    (&["winnipeg"], "winnipeg, manitoba, canada, north america"),
    (&["regina"], "regina, saskatchewan, canada, north america"),
    (&["halifax"], "halifax, nova scotia, canada, north america"),
    (&["fredericton"], "fredericton, new brunswick, canada, north america"),
    (&["st. john's, n", "st john's, n"], "st. john's, newfoundland, canada, north america"),
    (&["charlottetown"], "charlottetown, prince edward island, canada, north america"),
    (&["whitehorse"], "whitehorse, yukon, canada, north america"),
    (&["yellowknife"], "yellowknife, northwest territories, canada, north america"),

    // =========================================================================
    // UNITED KINGDOM — Major cities and film hubs
    // =========================================================================
    (&["london"], "london, england, united kingdom, uk, europe"),
    (&["manchester"], "manchester, england, united kingdom, uk, europe"),
    (&["birmingham, uk", "birmingham, england"], "birmingham, england, united kingdom, uk, europe"),
    (&["glasgow"], "glasgow, scotland, united kingdom, uk, europe"),
    (&["edinburgh"], "edinburgh, scotland, united kingdom, uk, europe"),
    (&["bristol"], "bristol, england, united kingdom, uk, europe"),
    (&["liverpool"], "liverpool, england, united kingdom, uk, europe"),
    (&["leeds"], "leeds, england, united kingdom, uk, europe"),
    (&["belfast"], "belfast, northern ireland, united kingdom, uk, europe"),
    (&["cardiff"], "cardiff, wales, united kingdom, uk, europe"),
    (&["pinewood"], "pinewood studios, buckinghamshire, england, united kingdom, uk, europe"),
    (&["leavesden"], "leavesden studios, hertfordshire, england, united kingdom, uk, europe"),
    (&["shepperton"], "shepperton studios, surrey, england, united kingdom, uk, europe"),

    // =========================================================================
    // GERMANY — Major cities and film hubs
    // =========================================================================
    (&["berlin"], "berlin, germany, europe"),
    (&["babelsberg"], "babelsberg, potsdam, brandenburg, germany, europe"),
    (&["potsdam"], "potsdam, brandenburg, germany, europe"),
    (&["munich", "münchen"], "munich, bavaria, germany, europe"),
    (&["hamburg"], "hamburg, germany, europe"),
    (&["frankfurt"], "frankfurt, hesse, germany, europe"),
    (&["cologne", "köln"], "cologne, north rhine-westphalia, germany, europe"),
    (&["düsseldorf", "dusseldorf"], "düsseldorf, north rhine-westphalia, germany, europe"),
    (&["stuttgart"], "stuttgart, baden-württemberg, germany, europe"),
    (&["leipzig"], "leipzig, saxony, germany, europe"),
    (&["dresden"], "dresden, saxony, germany, europe"),
    (&["hanover", "hannover"], "hanover, lower saxony, germany, europe"),
    (&["nuremberg", "nürnberg"], "nuremberg, bavaria, germany, europe"),
    (&["freiburg"], "freiburg, baden-württemberg, germany, europe"),

    // =========================================================================
    // FRANCE — Major cities and film locations
    // =========================================================================
    (&["paris"], "paris, île-de-france, france, europe"),
    (&["marseille"], "marseille, provence, france, europe"),
    (&["lyon"], "lyon, auvergne-rhône-alpes, france, europe"),
    (&["cannes"], "cannes, provence, france, europe"),
    (&["nice"], "nice, provence, france, europe"),
    (&["bordeaux"], "bordeaux, nouvelle-aquitaine, france, europe"),
    (&["toulouse"], "toulouse, occitanie, france, europe"),
    (&["strasbourg"], "strasbourg, alsace, france, europe"),

    // =========================================================================
    // ITALY
    // =========================================================================
    (&["rome", "roma"], "rome, lazio, italy, europe, cinecittà"),
    (&["milan", "milano"], "milan, lombardy, italy, europe"),
    (&["naples", "napoli"], "naples, campania, italy, europe"),
    (&["florence", "firenze"], "florence, tuscany, italy, europe"),
    (&["turin", "torino"], "turin, piedmont, italy, europe"),
    (&["venice", "venezia"], "venice, veneto, italy, europe"),

    // =========================================================================
    // SPAIN
    // =========================================================================
    (&["madrid"], "madrid, spain, europe"),
    (&["barcelona"], "barcelona, catalonia, spain, europe"),
    (&["seville", "sevilla"], "seville, andalusia, spain, europe"),
    (&["valencia"], "valencia, spain, europe"),
    (&["bilbao"], "bilbao, basque country, spain, europe"),
    (&["málaga", "malaga"], "málaga, andalusia, spain, europe"),

    // =========================================================================
    // SCANDINAVIA & NORDICS
    // =========================================================================
    (&["stockholm"], "stockholm, sweden, scandinavia, europe"),
    (&["gothenburg", "göteborg"], "gothenburg, sweden, scandinavia, europe"),
    (&["copenhagen", "københavn"], "copenhagen, denmark, scandinavia, europe"),
    (&["oslo"], "oslo, norway, scandinavia, europe"),
    (&["bergen"], "bergen, norway, scandinavia, europe"),
    (&["helsinki"], "helsinki, finland, scandinavia, europe"),
    (&["reykjavik", "reykjavík"], "reykjavik, iceland, europe"),

    // =========================================================================
    // BENELUX
    // =========================================================================
    (&["amsterdam"], "amsterdam, netherlands, europe"),
    (&["rotterdam"], "rotterdam, netherlands, europe"),
    (&["the hague", "den haag"], "the hague, netherlands, europe"),
    (&["brussels", "bruxelles"], "brussels, belgium, europe"),
    (&["antwerp"], "antwerp, belgium, europe"),
    (&["luxembourg"], "luxembourg, europe"),

    // =========================================================================
    // CENTRAL & EASTERN EUROPE
    // =========================================================================
    (&["vienna", "wien"], "vienna, austria, europe"),
    (&["zurich", "zürich"], "zurich, switzerland, europe"),
    (&["geneva", "genève"], "geneva, switzerland, europe"),
    (&["prague", "praha"], "prague, czech republic, europe"),
    (&["budapest"], "budapest, hungary, europe"),
    (&["warsaw", "warszawa"], "warsaw, poland, europe"),
    (&["krakow", "kraków"], "krakow, poland, europe"),
    (&["bucharest"], "bucharest, romania, europe"),
    (&["sofia"], "sofia, bulgaria, europe"),
    (&["athens"], "athens, greece, europe"),
    (&["thessaloniki"], "thessaloniki, greece, europe"),
    (&["istanbul"], "istanbul, turkey"),
    (&["ankara"], "ankara, turkey"),
    (&["belgrade"], "belgrade, serbia, europe"),
    (&["zagreb"], "zagreb, croatia, europe"),
    (&["dubrovnik"], "dubrovnik, croatia, europe"),
    (&["bratislava"], "bratislava, slovakia, europe"),
    (&["ljubljana"], "ljubljana, slovenia, europe"),
    (&["tallinn"], "tallinn, estonia, europe"),
    (&["riga"], "riga, latvia, europe"),
    (&["vilnius"], "vilnius, lithuania, europe"),
    (&["tbilisi"], "tbilisi, georgia, caucasus"),
    (&["kyiv", "kiev"], "kyiv, ukraine, europe"),

    // =========================================================================
    // PORTUGAL & IRELAND
    // =========================================================================
    (&["lisbon", "lisboa"], "lisbon, portugal, europe"),
    (&["porto"], "porto, portugal, europe"),
    (&["dublin"], "dublin, ireland, europe"),
    (&["galway"], "galway, ireland, europe"),

    // =========================================================================
    // AUSTRALIA & NEW ZEALAND
    // =========================================================================
    (&["sydney"], "sydney, new south wales, australia, oceania"),
    (&["melbourne"], "melbourne, victoria, australia, oceania"),
    (&["brisbane"], "brisbane, queensland, australia, oceania"),
    (&["perth, au", "perth, west"], "perth, western australia, australia, oceania"),
    (&["adelaide"], "adelaide, south australia, australia, oceania"),
    (&["canberra"], "canberra, australian capital territory, australia, oceania"),
    (&["gold coast"], "gold coast, queensland, australia, oceania"),
    (&["hobart"], "hobart, tasmania, australia, oceania"),
    (&["darwin, au", "darwin, northern"], "darwin, northern territory, australia, oceania"),
    (&["auckland"], "auckland, new zealand, oceania"),
    (&["wellington"], "wellington, new zealand, oceania"),
    (&["christchurch"], "christchurch, new zealand, oceania"),

    // =========================================================================
    // EAST ASIA
    // =========================================================================
    (&["tokyo"], "tokyo, japan, east asia, asia"),
    (&["kyoto"], "kyoto, japan, east asia, asia"),
    (&["osaka"], "osaka, japan, east asia, asia"),
    (&["seoul"], "seoul, south korea, east asia, asia"),
    (&["busan"], "busan, south korea, east asia, asia"),
    (&["beijing", "peking"], "beijing, china, east asia, asia"),
    (&["shanghai"], "shanghai, china, east asia, asia"),
    (&["hong kong"], "hong kong, china, east asia, asia"),
    (&["shenzhen"], "shenzhen, guangdong, china, east asia, asia"),
    (&["taipei"], "taipei, taiwan, east asia, asia"),

    // =========================================================================
    // SOUTH & SOUTHEAST ASIA
    // =========================================================================
    (&["mumbai", "bombay"], "mumbai, maharashtra, india, south asia, asia, bollywood"),
    (&["delhi", "new delhi"], "new delhi, india, south asia, asia"),
    (&["chennai", "madras"], "chennai, tamil nadu, india, south asia, asia, kollywood"),
    (&["hyderabad"], "hyderabad, telangana, india, south asia, asia, tollywood"),
    (&["bangalore", "bengaluru"], "bangalore, karnataka, india, south asia, asia"),
    (&["kolkata", "calcutta"], "kolkata, west bengal, india, south asia, asia"),
    (&["bangkok"], "bangkok, thailand, southeast asia, asia"),
    (&["singapore"], "singapore, southeast asia, asia"),
    (&["kuala lumpur"], "kuala lumpur, malaysia, southeast asia, asia"),
    (&["manila"], "manila, philippines, southeast asia, asia"),
    (&["jakarta"], "jakarta, indonesia, southeast asia, asia"),
    (&["ho chi minh", "saigon"], "ho chi minh city, vietnam, southeast asia, asia"),
    (&["hanoi"], "hanoi, vietnam, southeast asia, asia"),
    (&["phnom penh"], "phnom penh, cambodia, southeast asia, asia"),

    // =========================================================================
    // MIDDLE EAST
    // =========================================================================
    (&["dubai"], "dubai, uae, united arab emirates, middle east"),
    (&["abu dhabi"], "abu dhabi, uae, united arab emirates, middle east"),
    (&["tel aviv"], "tel aviv, israel, middle east"),
    (&["jerusalem"], "jerusalem, israel, middle east"),
    (&["doha"], "doha, qatar, middle east"),
    (&["riyadh"], "riyadh, saudi arabia, middle east"),
    (&["jeddah"], "jeddah, saudi arabia, middle east"),
    (&["amman"], "amman, jordan, middle east"),
    (&["beirut"], "beirut, lebanon, middle east"),
    (&["muscat"], "muscat, oman, middle east"),
    (&["kuwait city"], "kuwait city, kuwait, middle east"),
    (&["bahrain", "manama"], "manama, bahrain, middle east"),

    // =========================================================================
    // AFRICA
    // =========================================================================
    (&["lagos"], "lagos, nigeria, west africa, africa"),
    (&["abuja"], "abuja, nigeria, west africa, africa"),
    (&["nairobi"], "nairobi, kenya, east africa, africa"),
    (&["cape town"], "cape town, western cape, south africa, africa"),
    (&["johannesburg"], "johannesburg, gauteng, south africa, africa"),
    (&["durban"], "durban, kwazulu-natal, south africa, africa"),
    (&["cairo"], "cairo, egypt, north africa, africa"),
    (&["casablanca"], "casablanca, morocco, north africa, africa"),
    (&["marrakech", "marrakesh"], "marrakech, morocco, north africa, africa"),
    (&["accra"], "accra, ghana, west africa, africa"),
    (&["dakar"], "dakar, senegal, west africa, africa"),
    (&["addis ababa"], "addis ababa, ethiopia, east africa, africa"),
    (&["kampala"], "kampala, uganda, east africa, africa"),
    (&["dar es salaam"], "dar es salaam, tanzania, east africa, africa"),
    (&["tunis"], "tunis, tunisia, north africa, africa"),
    (&["algiers"], "algiers, algeria, north africa, africa"),
    (&["kigali"], "kigali, rwanda, east africa, africa"),
    (&["luanda"], "luanda, angola, africa"),
    (&["maputo"], "maputo, mozambique, africa"),
    (&["windhoek"], "windhoek, namibia, africa"),

    // =========================================================================
    // LATIN AMERICA & CARIBBEAN
    // =========================================================================
    (&["mexico city", "ciudad de méxico", "cdmx"], "mexico city, mexico, latin america, north america"),
    (&["guadalajara"], "guadalajara, jalisco, mexico, latin america"),
    (&["cancún", "cancun"], "cancún, quintana roo, mexico, latin america"),
    (&["são paulo", "sao paulo"], "são paulo, brazil, latin america, south america"),
    (&["rio de janeiro"], "rio de janeiro, brazil, latin america, south america"),
    (&["buenos aires"], "buenos aires, argentina, latin america, south america"),
    (&["bogotá", "bogota"], "bogotá, colombia, latin america, south america"),
    (&["medellín", "medellin"], "medellín, colombia, latin america, south america"),
    (&["lima"], "lima, peru, latin america, south america"),
    (&["santiago, chile", "santiago, cl"], "santiago, chile, latin america, south america"),
    (&["havana", "la habana"], "havana, cuba, caribbean, latin america"),
    (&["san juan, pr", "san juan, puerto"], "san juan, puerto rico, caribbean"),
    (&["kingston, jamaica"], "kingston, jamaica, caribbean"),
    (&["panama city"], "panama city, panama, central america, latin america"),
    (&["san josé, costa", "san jose, costa"], "san josé, costa rica, central america, latin america"),
    (&["montevideo"], "montevideo, uruguay, latin america, south america"),
    (&["quito"], "quito, ecuador, latin america, south america"),
    (&["caracas"], "caracas, venezuela, latin america, south america"),

    // =========================================================================
    // PACIFIC ISLANDS
    // =========================================================================
    (&["fiji", "suva"], "suva, fiji, pacific islands, oceania"),
    (&["samoa", "apia"], "apia, samoa, pacific islands, oceania"),
];
