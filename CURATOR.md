# The Curator: IA Darkroom's AI engine

The Curator is the part of IA Darkroom that does what I did by hand in your Photos library last
week, on the whole library, overnight, and keeps doing it as new photos arrive: it groups, sorts,
stacks, culls, grades, finds the known bests, builds the trips, moves Memories out of your way,
and tells you what it sees. Apple Photos stays the system of record. The Curator writes to the
catalog first, you review, then it pushes.

Three rules it never breaks:

1. Nothing is deleted. Ever. The strongest thing it does is hide, and hiding is reversible.
2. Review before write. Every run ends in a contact sheet with counts. You accept, fix or throw it
   away. Nothing reaches Photos until you say so.
3. Your pictures stay on your Mac unless you send them. Two of the three layers below are fully
   local. The third (judgment) sends 1024 px thumbnails of candidates to Claude, only when you run
   it, with the count and cost shown first.

## What it answers

| You ask | The Curator does |
|---|---|
| "What's in here and what's wrong with it?" | A library report: counts by year, kind, trip, device; duplicates; junk; albums that overlap; what it would do first |
| "Get rid of the obviously bad shots" | Cull: blur, missed focus, black frames, blown frames, pocket shots, screenshots and receipts, 14 copies of the same menu |
| "One winner per burst" | Stacks: bursts and near-duplicates collapse to one stack with the best frame on top |
| "Sort by what it is" | Kinds: landscape, city and street, arches and thresholds, people, events and performances, travel snapshots, documents, food, pets, junk |
| "Make the trip albums" | Trips: time and place clustering, named by place and dates, in the `01_` structure you use |
| "I don't want to see those" | Memories: photos of named people, events or date ranges leave the Years view, into a locked Memories view in Darkroom |
| "Which are the good ones?" | Grade: the Infinite Arch matrix, a tier (S, A, B, C), and a sentence of why |
| "Find The Sentinel" | Known bests: the pieces and series we named in the curator chats, matched to the frames in the library |
| "Which of these twelve should I print?" | Ask: a conversation about the selection, with the brand's voice |

## How it works: three layers

The design is a funnel. Cheap signals handle the whole library. On-device vision handles what
survives. Claude, the expensive and slow layer, only sees the few percent that need judgment.

### Layer 1: Signals (local, free, seconds per thousand)

Everything Apple and the camera already know, read through the helper:

- PhotoKit: creation date, location, favorite, hidden, edited, burst id, media subtype
  (panorama, screenshot, Live, RAW), pixel size, duration, the album and folder it lives in.
- EXIF from the original or the preview: camera, lens, focal length, exposure, ISO, orientation.
- Photos' own analysis, read from the library's `Photos.sqlite` (read only, copied first, never
  opened in place): scene classifications, people and face counts, the aesthetic and curation
  scores Photos computes for its own Memories, "is utility" (screenshots, receipts, whiteboards).
  This reader is one small swappable part. Its table layout changes between macOS versions; when
  it does not match, the Curator falls back to Layer 2 and says so.
- Darkroom's own quick measures on the 1024 px preview: sharpness (Laplacian variance), exposure
  histogram (clipped black or white), a perceptual hash for exact and near duplicates.

### Layer 2: Vision (local, Apple Neural Engine, about 40 ms per photo)

Apple's Vision and Core ML frameworks, in the helper, on Photos' own 1024 px previews (no
originals are downloaded for this):

- `VNClassifyImageRequest`: 1,300 scene and object labels with confidences.
- `VNGenerateImageFeaturePrintRequest`: a feature print per photo; distance between prints is how
  near-duplicates and "same scene, different moment" are found.
- `VNCalculateImageAestheticsScoresRequest` (macOS 15 and later): an overall aesthetic score and
  an "is utility" flag.
- Face detection and face count (no identification; people are matched by Photos' own People
  names in Layer 1, which is where your tagging lives).
- Horizon and saliency, for the composition features the grade uses.
- A CLIP-style image and text embedding model (MobileCLIP, Apple's open weights, converted to
  Core ML, 512 dimensions): this is what makes "neon street at night, no people" a search and
  what the kind classifier is built on (zero-shot prompts per kind, then your corrections).

Everything from Layers 1 and 2 is stored per photo, keyed by Apple's cloud identifier, so the
same photo is known on both Macs.

### Layer 3: Judgment (Claude, your API key, only on candidates)

What a model that has read the Infinite Arch mentor document can do that a classifier cannot:
the ten-dimension score, the tier, the title and series suggestions, the one-line reason, the
library report in sentences, the answers in Ask.

- Runs on 1024 px thumbnails, in contact sheets of 12 per request with the frame numbers
  burned in, so cost is roughly a cent or two per photo at today's prices. A whole 65k library
  never goes: the funnel sends a few percent (favorites, edits, stack winners, anything Layer 2
  scores high, anything in a kind you grade, anything you select).
- The system prompt is the mentor/curator document: the brand, the arches and thresholds,
  "photograph the invitation, not the thing itself", the Ma / yohaku direction, the scoring
  matrix (Composition, Light, Visual Weight, Emotional, Symbolism, Storytelling, Printability,
  Gallery, Commercial, Originality, Longevity), the tiers, the rule that Velvia is valid and
  panoramics are in-camera 65:24. Plus your last 200 overrides as examples, so it learns your
  taste rather than a generic one.
- Returns JSON only: per frame, scores, tier, kind, confidence, reason, suggested title and series
  (only for S and A), flags (print candidate, show candidate).
- The key lives in the macOS Keychain. Nothing is sent without a run you started, and every run
  shows "412 photos to Claude, about $6" before it goes. A "local only" switch turns the layer
  off; everything else still works.

## The jobs

Each job is an engine command (`curator.*`), journaled, resumable, and runs on a worker with a
live count in the window. All of them read Layers 1 and 2 from the store and only compute what is
missing, so the second run of anything is fast.

**curator.scan**: fills the store. First run on 65k photos: Layer 1 in under ten minutes, Layer 2
in about 45 minutes on an M1 Max (it uses the previews Photos already has; photos that only exist
in iCloud get the preview fetched, not the original). After that, incremental: new and changed
photos only, checked on launch and on a timer.

**curator.stacks**: bursts (Apple's burst id), exact duplicates (hash), near-duplicates (feature
print distance under a threshold, same minute). Each stack gets a winner: sharpest, best exposed,
faces with eyes open when there are faces, Photos' aesthetic score as the tie-breaker, your
favorite or edit always wins. Writes `Stack:` membership to the catalog; in Photos, stack losers
get the keyword `Pick: No` and nothing else, unless you choose to hide them.

**curator.cull**: marks junk. Rules, each with a confidence, each overridable: blur below the
threshold for its kind (a long exposure is not blur), black or white frames, utility images
(screenshots, receipts, whiteboards, documents) unless they are in a Documents album already,
pocket and accidental frames, photos of a screen, more than N near-identical frames. Output is a
review sheet; accepted frames get `Pick: No` and `Kind: Junk`, and stay visible in Photos unless
you turn on "hide junk".

**curator.kinds**: one primary kind per photo, confidence, and secondary labels. The kinds are
yours and editable in the rules file; the defaults are Landscape, City and street, Arches and
thresholds, People, Events and performances, Travel snapshot, Documents, Food, Pets, Junk. Built
from Layer 2 labels plus zero-shot CLIP prompts per kind, then corrected by a small classifier
trained on your overrides (a logistic model on the embeddings, retrained in seconds). Writes
`Kind:` keywords and keeps the Kind smart collections current.

**curator.trips**: clusters by time gap and distance (default: a new trip after 36 hours with no
photos, or 150 km from the last cluster; home radius 50 km is never a trip). Names each trip by
reverse geocoding its places (CLGeocoder, local) and its dates: "New Orleans, Mar 14 to 17 2025".
Day trips stay trips if they are away from home. Proposes the album tree in your structure:

```
01_Trips/
  2025/
    01_New Orleans, Mar 14 to 17 2025/
      SELECTS   MEH   NO
```

and the move of every pre-existing album into `OLD Do not delete/`, as a plan you review. Writes
`Trip:` keywords; albums are mirrored collections, so they stay in sync when grades change.

**curator.memories**: a rule set, not a model. Rules name people (from Photos' People names),
date ranges, events or kinds, and say visible, Memories or hidden. Your current rules are the
defaults: couple shots and singles of the person you named go to hidden; family shots stay
visible; theatre productions go to hidden; dance, cheer and singing performances stay visible;
unsure days stay visible. A photo matched by a Memories rule gets `Memories:` and leaves the
Years view (hidden in Photos, with the Hidden-album lock on), and appears in Darkroom's locked
Memories view, categorized (theatre vs the rest). Every move is listed before it happens, with
faces blurred on the review sheet if you want.

**curator.grade**: the Infinite Arch grade, on candidates (stack winners, favorites, edits, high
Layer 2 scores, any selection). Layer 3. Writes `Tier:` (S, A, B, C) and the ten scores to the
catalog, `Series:` where it suggests one, and marks Print and Show candidates. `Pick:` is yours:
the grade proposes 1, 2, 3 and you confirm with the number keys. S and A frames get a suggested
*Where…* title, which you accept or edit before it goes to Photos as the title.

**curator.bests**: the known bests. Three ways to find them: titles and series from our curator
chats (a list you paste or I load from the project), the exported finals in your Lightroom or
Photoshop folders (perceptual hash match back to the Photos original, including through crops),
and anything already titled in Photos. Writes title, `Series:` and `Tier:` and builds the
`00_Known Bests` album.

**curator.report**: the library in sentences and a few tables: how many, by year, kind, trip
and camera; how much is junk and duplicates; albums that overlap; what it would do first and how
long it would take. Layer 3 writes the sentences from Layer 1 and 2 numbers; the numbers never
come from the model.

**curator.ask**: a conversation about the current selection or the current folder, with the
contact sheet attached: "which of these for the show", "what series do these make", "what is this
photo missing". Answers can become actions ("mark these three Select") with the same review step.

**curator.apply**: the write step shared by all of them. Shows the plan (counts per change, a
contact sheet per group), writes to the catalog, then `photos.push` carries it to Photos through
the helper: favorites, hidden, keywords in the fixed families (`Pick:`, `Trip:`, `Kind:`,
`Series:`, `Tier:`, `Memories:`, `Stack:`), titles and captions, album and folder membership.
Checked counts after every write, retried once, logged.

## The rules file

`curator-rules.json` in the library folder (so both Macs share it), editable in the app:

```json
{
  "home": {"lat": 33.15, "lon": -96.82, "radiusKm": 50},
  "trips": {"gapHours": 36, "jumpKm": 150, "prefix": "01_"},
  "kinds": ["Landscape", "City and street", "Arches and thresholds", "People",
            "Events and performances", "Travel snapshot", "Documents", "Food", "Pets", "Junk"],
  "cull": {"blur": 0.35, "nearDupMax": 4, "hideJunk": false},
  "memories": [
    {"people": ["<name>"], "with": "alone or couple", "action": "hidden"},
    {"people": ["<name>"], "with": "family", "action": "visible"},
    {"event": "theatre production", "action": "hidden", "category": "Theatre"},
    {"event": ["dance", "cheer", "singing"], "action": "visible"}
  ],
  "grade": {"tiers": ["S", "A", "B", "C"], "titleFor": ["S", "A"], "edition": 25},
  "judgment": {"enabled": true, "maxPerRun": 2000, "thumb": 1024, "sheet": 12}
}
```

## What it stores

`curator.sqlite` in the library folder, keyed by cloud identifier:

- `photo`: the Layer 1 row (dates, place, camera, flags, Photos' scores when readable)
- `vision`: labels with confidence, aesthetic score, utility flag, face count, horizon
- `embedding`: the 512-d CLIP vector (float16, about 1 KB per photo; 65 MB for the library) and
  the Vision feature print
- `stack`, `trip`, `kind`, `memory`: the groupings with the version of the rules that made them
- `grade`: the ten scores, tier, reason, suggested title and series, model and prompt version
- `decision`: every write the Curator proposed, whether you accepted it, and your override; this
  table is what the taste model and the Claude examples are built from

Thumbnails live in `~/Library/Application Support/Darkroom/thumbs/` on each Mac, not in iCloud.

## Interface

A **Curator** workspace next to Organize, Import and Print, same three columns:

- Left: the jobs, each with its last run, and the rules.
- Middle: the review sheet of the current run: groups (for instance "Blur, 312", "Screenshots,
  1,040", "Stack losers, 2,215"), each a contact sheet. Space previews, number keys override the
  grade, X removes a frame from the plan, A accepts a group.
- Right: for the frame under the cursor, why: the signals, the labels, the scores, the sentence.
  Below it the plan totals and one button: Apply to Photos.

Elsewhere in the app the Curator shows up as: a `Curator` column of the grid (tier and kind
badges, stack counts), a search field that takes words ("fog over water, no people"), and the
Ask drawer on any selection. Every run has a live count in the window and can be left running.

## What it learns

Every override is a training example. Two things use them:

- The local taste model: a logistic classifier per decision kind (cull, kind, stack winner) on
  the embeddings, retrained in seconds after each review. After a few hundred corrections it
  agrees with you more than the zero-shot prompts do.
- Claude's examples: the most recent and the most contradicted overrides go into the grading
  prompt as examples, with your reason when you typed one.

Both can be reset. Neither leaves the Mac except as the examples in a grading request.

## Build order

Hours are rough engineering estimates, about 120 to 170 in total. The Organize workspace
(milestone 4 in Spec v2) is what the Curator writes into, so it goes first.

| # | Milestone | You can do this when it lands | Hours |
|---|---|---|---|
| C0 | Store and scan, Layer 1 | A library report with real numbers; search by date, place, camera | 10 to 15 |
| C1 | Stacks and cull | One winner per burst, junk marked, reviewed, pushed as `Pick: No` | 15 to 20 |
| C2 | Vision, Layer 2 | Kinds, near-duplicates, word search, aesthetic scores, trained by your corrections | 25 to 35 |
| C3 | Trips and the album tree | Every trip as an album in the `01_` structure, old albums moved to `OLD Do not delete`, reviewed | 15 to 20 |
| C4 | Memories | The people and event rules applied, hidden view inside Darkroom, nothing deleted | 10 to 15 |
| C5 | Judgment, Layer 3 | Infinite Arch grades, tiers, titles and series, Ask, the report in sentences | 25 to 35 |
| C6 | Known bests and learning | The named pieces found and titled; overrides feeding both models | 12 to 18 |
| C7 | Curator workspace polish | Review sheets, keys, badges in the grid, incremental runs on launch | 10 to 15 |

## What I need from you

- The mentor/curator instruction document (the current version), to be the Layer 3 prompt
- The list of named pieces and series from the curator chats, or a yes to loading them from the
  project
- The folder where your Lightroom and Photoshop finals are exported, for the known-bests match
- The names, as Photos has them, of the people in the Memories rules
- Your home location for the trips rule (default: Frisco, 50 km)
- Whether junk should be hidden in Photos or only marked `Pick: No` (default: marked only)
- The macOS version on both Macs (the aesthetics request needs macOS 15)
- Your Anthropic API key, entered once in the app, stored in the Keychain, never in chat
