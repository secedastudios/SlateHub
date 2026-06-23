# Expected breakdown — `test_script.fountain`

Use this as a checklist when validating either the raw `.fountain`
upload or its PDF export. Tier 0 (native FDX tags) is N/A here because
the source is Fountain — tier 1 (structural) and tier 2 (dictionary +
ALL-CAPS introductions) carry the load.

## Title page

| Field | Expected |
|---|---|
| `title` | `THE LAST DEPOSIT` |
| `writers` | `["Test Author"]` |
| `draft_date` | something matching `/draft|revision/i` — exact text varies |
| `contact_info` | `test@example.com` |

## Scenes

Four scenes; numbering is sequential.

### Scene 1 — INT. DETECTIVE'S APARTMENT - NIGHT

- `int_ext`: `INT`
- `location`: `DETECTIVE'S APARTMENT`
- `time_of_day`: `NIGHT`
- `speaking_cast`: `["MAYA"]`
- `cast`: `["MAYA"]`
- `props`: should include `Bourbon` (via dict — actually `bourbon` isn't in our prop dict — see notes), `Laptop`, `Revolver`, `Briefcase`, `Phone`, `Keys`
- `wardrobe`: `["Coat"]`
- Per-element chips:
  - Scene heading paragraph carries `Int/Ext`, `Location`, `Time of Day` chips
  - Each Character cue (MAYA) carries a `Cast` chip
  - Action paragraphs carry Props chips on the dictionary hits

### Scene 2 — EXT. WAREHOUSE DISTRICT - CONTINUOUS

- `int_ext`: `EXT`
- `location`: `WAREHOUSE DISTRICT`
- `time_of_day`: `CONTINUOUS`
- `speaking_cast`: `["MAYA"]`
- `cast`: `["MAYA"]`
- `props`: `["Pistol"]` (note: `Gun` may not match because the line says "pistol drawn")
- `extras_background`: `["Crowd"]` (from "A crowd of dockworkers")
- `animals`: `["Dog"]`
- `special_effects`: `["Fog"]`
- Tier 2 should produce a `Vehicle: Sedan` chip on the action paragraph

### Scene 3 — INT. WAREHOUSE - MOMENTS LATER

- `int_ext`: `INT`
- `location`: `WAREHOUSE`
- `time_of_day`: `MOMENTS LATER`
- `speaking_cast`: `["VIKTOR", "MAYA"]`
- `cast`: `["VIKTOR", "MAYA"]` (Maya speaks; Viktor speaks)
- `props`: `["Cigar", "Pistol"]` (and possibly `Crate` — depends on dict)
- `wardrobe`: `["Suit"]`
- `extras_background`: `["Guards"]`
- `special_effects`: `["Explosion", "Smoke", "Debris"]` (plus possibly `Explodes`/etc.)
- Per-element chips:
  - The first action paragraph should get:
    - `Introduction: Viktor` + `Description: Viktor: 50s, weathered with a scar across his cheek, sits behind a desk in a tailored suit, smoking a cigar`
    - `Introduction: Guards` + `Description: Guards: flank him, rifles ready`
    - `Cast: VIKTOR` chip from mentioned-character scan (VIKTOR is a known character — has dialogue cue later)
  - `Foolish.` in Viktor's dialogue should render as `<em>Foolish.</em>` (Fountain italic via `*…*`)
  - `Goodbye.` at the end should render as `<strong>Goodbye.</strong>` (Fountain bold via `**…**`)
- Bold/italic markdown markers in the underlying element text should survive a round-trip through FDX export.

### Scene 4 — EXT. ROOFTOP - DAY

- `int_ext`: `EXT`
- `location`: `ROOFTOP`
- `time_of_day`: `DAY`
- `speaking_cast`: `["MAYA"]`
- `cast`: `["MAYA"]` (Maya is mentioned in action, has a Character cue)
- `props`: `["Photograph", "Lighter"]` (and possibly `Photo` if dedupe doesn't merge)
- `wardrobe`: `["Dress"]`
- `extras_background`: `["Crowd"]`
- Vehicle chip on action: `Vehicle: Helicopter`
- The action paragraph containing `Maya stands at the edge` should get a `Cast: MAYA` chip from the mentioned-character scan.

## Cross-scene checks

- **Embedding dedupe (if Ollama is up)**: `Photograph` and `Photo` should collapse to one canonical form (likely `Photograph` — the longer of the two).
- **Schedule**: 4 scenes total; bin-packed into shoot days by location. With the default `DAILY_PAGE_TARGET=5.0`, expect 1 shoot day for this short script.
- **Production graph**: nodes for 4 unique locations, 2-3 unique characters, ~15 unique props.

## What success looks like end-to-end

1. Upload either `test_script.fountain` or a PDF export of it.
2. Job reaches `Done` without errors.
3. `/jobs/:id/script` renders the screenplay with:
   - Solid blue `Cast: MAYA / VIKTOR` chips on every Character cue.
   - Solid `Int/Ext / Location / Time of Day` chips on every scene heading.
   - Dashed tier-2 chips on action paragraphs for props, wardrobe, vehicles, animals, SFX, extras.
   - `Introduction` + `Description` chips on the action paragraph that introduces VIKTOR.
   - Italic / bold rendered as `<em>` / `<strong>` (not literal `*…*`).
4. Breakdown report shows cast lists matching the speaking-cast checklist above.
5. Schedule populates with the right locations.
6. FDX export downloads a valid `.fdx` that re-opens in Final Draft, with `TagCategories` populated for cast/props/etc.

## Known limitations to expect

- **`Bourbon`** is not in our Props dictionary; it won't auto-tag. Add manually if you care.
- **`Crate`, `Hook`, `Keys`** are borderline — some hit the dict, some don't. Not a regression.
- **`Glass`** appears in the prop dict but the script says "glass of bourbon" — the dict match is on the word "glass" alone, so it'll tag as Props (correct: a drinking glass is a prop).
- **`Scar`** and **`Wind`** are NOT in our dicts (good — too generic).
- If you upload the **PDF** rather than the `.fountain`, italic/bold round-trip is not preserved unless your Fountain→PDF tool exports a "tagged" PDF (most don't). The bold/italic round-trip is FDX-only.
