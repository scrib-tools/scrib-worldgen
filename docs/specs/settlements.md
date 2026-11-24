# Settlements: growing and founding towns (ST)

> Status 2026-11-23: spec + first slices. Owner: "generate settlements, or expand settlements (like make Balmora 10
> times bigger)". Research: Scrib `docs/research/settlements-magna-urbis-2026-11-23.md` (Magna Urbis fit, `path:line`)
> and `docs/research/settlements-industry-2026-11-23.md` (techniques, prior art, crates). Read "Contract", then your slice.

## Goal

Grow existing towns and found new ones, Morrowind first, generic in the tool (no Morrowind names, units or IDs here;
Scrib's exporters convert). Built on **Magna Urbis** (github.com/dunkean/magna-urbis, AGPL-3.0-only Rust): it already
has the whole chain we need (site survey, field-traced organic streets, blocks, walls, bridges, lots, a programme of
uses, building requests, 115 cultures as data) and a hook for kept content: a neighbour `Claim`
(`rust/crates/core/src/urban/neighbours.rs:21`), land a town never builds on plus edge streets it joins. We add what it
lacks: a kept town as the seed, Morrowind styles, and prefab kit placement instead of its procedural volumes.

## First comparison (owner 2026-11-23)

The 10x world's Balmora is misplaced (far from the new Odai; it must straddle it), so two arms, judged side by side:

- **A, virtual Balmora extended.** Vanilla Balmora on its own vanilla terrain, alone in an empty scene (no other
  world content), kept 1:1 as a `Claim`; Magna Urbis grows a town about 10x its built area around it, joining its
  exits (roads to Caldera, Pelagiad, Fort Moonmoth, the silt strider) and both banks of the Odai.
- **B, Balmora 2.0.** No old part. A new town founded on both banks of the new Odai in the 10x world (`x10s` guide),
  on the reach nearest where Balmora maps to, heights without Balmora's anchor stamp.

Each arm runs a few Magna Urbis cultures (arms beat repeats; the owner rates by elimination). **Milestone 1:** 2D
layout sheets (streets by rank, blocks, lots, kept footprints, water), the industry's and Magna Urbis's own review
step. **Milestone 2:** the same layouts in game with Morrowind buildings on the lots (A as an isolated scene, B in the
10x world).

**Relief (ST-3d, 2026-11-23).** Vanilla Balmora's ground is far steeper than a street can climb: median slope 0.35
under its footprints, 0.49 within 300 m, against Magna Urbis's 0.12 street limit (`urban/config.rs:342`); no quarter
street routes and the plan collapses. Arm A therefore plans on relief scaled about the water level (`relief_scale`
0.25 in `recipes/balmora-vanilla.toml`) and drops swamp pools under 2500 m2 (`min_water_area_m2`). This is a planning
stand-in for **terrain grading**, the standard step under generated towns (CityEngine terrain alignment, Houdini
city tools, the Procedural Tamriel cityforge grading pass): Milestone 2 must grade the game's terrain under the new
quarters to the planned heights, blending to the untouched ground under the kept town, and fill the dropped pools,
or the layout and the ground disagree. Arm B (ST-3e) needs the same: the 10x Odai runs in a gorge (banks 23-25 m,
slope ~0.21 within 300 m of the channel; the 0.083 site median hid it), so `relief_scale` 0.25 there too (0.5 left
every culture on one bank). Magna Urbis bridges wide water only on a town's founding road crossing
(`growth.rs:219-266`; its own crossings stop at ~21 m, `crossings.rs:133`), and the region stage normally supplies
that road: `settle` builds it (`crossing.rs`, recipe `[crossing]`) across the narrowest water near the centre.

## Contract: files only

Scrib (GPL) writes a **site folder**; this tool (AGPL) reads it and writes a **layout folder**; later a placements
file becomes a generated mod that Scrib loads like any mod ("everything is a mod"). Neither side links the other.

Site folder, format 1 (metres, x east, y north, raster row 0 = north edge):
- `site.json`: `format: 1`, `name`, `producer`, `origin_m: [x, y]` (world position of the north-west corner of
  raster cell (0,0)), `cell_m` (raster spacing), `dims: [w, h]`, `water_level_m`, `centre_m: [x, y]` (where the
  town's centre should be), `source` (free text, e.g. "vanilla Balmora" or "x10s guide"), optional `claim`
  (below), `blake3` of each raster as `{ "heights.f32": "<hex>", "water.u8": ..., "roads.u8": ... }` (keyed by file
  name; `settle` refuses a mismatch).
- `heights.f32` (little-endian f32, row-major, `w*h`, metres), `water.u8` (1 = open water), `roads.u8` (1 = ground
  textured as road or path, optional).
- `claim` (arm A only): `footprints: [{ id, kind, polygon: [[x, y], ...] }]` with `kind` one of `building`, `wall`,
  `bridge`, `stairs`, `other` (the exporter's classification; polygon = convex hull of the reference's mesh vertices
  projected on the ground, world-transformed); `doors: [{ id, at: [x, y], facing_deg }]` later (Milestone 2).
The tool, not the exporter, derives Magna Urbis's `Claim` from it (land = footprints closed over the streets between
them, exits = road texels crossing the land edge): generic logic lives in the tool.

Layout folder: `layout.json` (Magna Urbis layout + partition, serde as they define it, plus our `recipe` and seed),
`sheet.png` (the 2D sheet), `stats.json` (built area new / kept, lot count by use, street length by rank, run time).

**Milestone 2 files, format 1** (types and readers `crates/settle/src/formats.rs`, `lots.rs`; every reader checks
`format` first and rejects unknown fields; world frame = metres, x east, y north, z up; yaw counter-clockwise about
+z from above; world = pos + R(yaw)·local; quarter turns exact):
- `kit.json` (Scrib writes): per model `id` (unique, Scrib's own), `kind` (one of `building`, `wall`, `arch`,
  `stairs`, `bridge`, `canal`, `lamp`, `stall`, `yard`, `retaining_wall`, `prop`), `tags`, `roles`, `footprint`
  (local CCW hull, >= 3 points; local x right, y front = door side), optional `obb`, `doors`
  (`at`, `facing_deg` local, 0 = +x CCW, `width_m`, `sill_z_m`), `height_m`, optional `floors`, `base_offset_m`,
  optional `foundation_depth_m`, `yaw_step_deg`, `weight`, `blake3`.
- `lots.json` (the tool writes, per culture): see ST "PL-0 as built".
- `placements.json` (the tool writes): `lots` (status `filled|merged|filler|empty`, optional `role`, `pad_z_m`,
  `front`, `merged_from`), `instances` (`kit` id, optional `lot`, `pos` [x,y,z], `yaw_deg`, optional `role`, `door`),
  `streets` (rank, role, polyline with planned z, one width per vertex, ground class), `features`
  (`retaining_wall|stairs|quay` polylines with base/top z), optional `grade` (file name).
- `grade.json` + `grade.f32` (planned heights) + `grade_w.f32` (weight 0..1) + `ground.u8` (class; `ground_legend`
  list, index = value, entry 0 `none`; default none, street_rank0..4, plaza, yard, quay, verge): grid aligned with
  Scrib's terrain stamp format v1 (`origin_m` = centre of sample (0,0), row 0 north, `spacing_m`, `dims`), blake3 of
  all three rasters required, cut/fill and blend band in the header.

## Tunables

Recipe files (`recipes/*.toml`, data): culture list, target population or built-area ratio (A: 10x the kept built
area, Guess), seed, crop radius, raster spacing. Every value carries its unit and source (`Guess` allowed), as in
Scrib's `tunables.md`. Magna Urbis's own options stay in its config, set from the recipe.

## Reuse check and industry comparison

Done in the two research notes. Summary: only growth-from-seed methods (Weber et al. 2009, agent-based growth,
constrained tensor fields) take a kept town as input; Magna Urbis is a field-traced generator with a claim hook, the
closest working implementation; CityEngine and UE5 City Sample place whole prefabs by rule tables on lots, the
pattern for Milestone 2; Daggerfall built towns from prefab blocks. Crates for our own geometry: `geo`, `spade`,
`i_overlay`, `cavalier_contours`. How Magna Urbis is used: git dependency pinned to a commit
(`21e22171f0b1766650b2cfef5d4bdaf574a1d325`) for the spike, then a vendored copy with our patches (Morrowind
cultures need a `catalogue.json` entry and a Rust table row; a kept-core input), per the research note's ranking.

## Slices (one deliverable each)

- **ST-1 (Scrib, GPL): site export of vanilla Balmora.** A `bevyrrowind worldgen-site` subcommand writing a site
  folder from the vanilla exterior: heights, water, roads, and the claim of one named anchor (Balmora), everything
  else left out (no doors yet). Test: round-trip of the format, Balmora's footprint count printed, water on both
  sides of its centre.
- **ST-2 (Scrib, GPL): site export from the 10x guide.** Same command, source `x10s`: a crop around the Odai reach
  nearest Balmora's mapped position, heights without the anchor stamp, upsampled to the site spacing; no claim.
- **ST-3a (this repo): `settle` CLI on Magna Urbis.** Reads a site folder + recipe, builds Magna Urbis's surface and
  site, runs quarters and parcels, writes the layout folder and sheet. Test: a synthetic site fixture (slope, straight
  river) gives streets and lots on both banks, identical output for one seed.
  *As built 2026-11-23:* `crates/settle` (lib + bin). The site is cropped to a square (`crop_radius_m`, shrunk to
  fit) and resampled to `raster_spacing_m`; open water gets one Magna Urbis class (`water_class`: sea 1, lake 2,
  river 3). `layout.json` keeps Magna Urbis's local frame (x east, y **south**) plus a `frame` to world metres;
  `built_area_m2` = land cut into lots. Fixture (2 km crop, 3000 people): 0.25-0.6 s per culture.
  *Fix 2026-11-23 (ST-3d), vanilla Balmora gave 1 lot:* its ground is game-steep (median slope 0.35 under the town,
  0.49 within 300 m) against Magna Urbis's 0.12 street grade (`urban/config.rs:342`), so no quarter street routes
  (`macroplan.rs:1481`, `:1604`), feeders fail, quarters are released (`quarters/service.rs:283`) and one contour-rescued
  quarter stays a reserve (`:224`). Two recipe tunables now shape the input: `relief_scale` (heights about the water
  level; arm A 0.25 in `recipes/balmora-vanilla.toml`, 1.0 for real-grade terrain) and `min_water_area_m2` (water
  bodies under 2500 m² plan as dry: swamp pools cut medina's west bank). Test `tests/steep_coast.rs`.
- **ST-3b (this repo): the kept town as a claim.** Land from the footprints, exits from road texels at the land edge,
  passed as Magna Urbis's `Claim`. Test: fixture with three footprints and one road: no lot inside the land, a street
  joins the exit.
  *As built 2026-11-24:* `crates/settle/src/claim.rs`, recipe table `[claim]` (12 tunables). Land: footprints
  rasterized (1 m), grown 4 m, closed 12 m (exact Euclidean distance transform), holes < 2500 m² filled, pieces
  < 200 m² dropped, traced and simplified 2 m. Exits: road texels just outside the land next to one inside, clustered,
  traced 60 m out and 60 m in along the road texels, merged within 20 m; passed as `Claim::streets` (Magna Urbis adds
  them to the roads it grows against and ties its dead ends to them, `neighbours::roads`, `join`; regional roads are
  only cut). The site is not `fixed_sites`: a fixed centre on kept land cannot seed growth (`growth.rs:322`), so Magna
  Urbis starts from the nearest free ground (Balmora: 30 m off). Test `tests/claim.rs`. Balmora: one land piece of
  27 267 m² (over the Odai), 5 exits all joined; population 3000 gives new lots / kept land 10.5 / 16.1 / 11.0.
- **ST-4 (runs): the Milestone 1 sheets** for arms A and B, a few cultures each, handed to the owner.
- **Later:** Morrowind cultures (vendored Magna Urbis); prefab kit catalogue from Morrowind's meshes (footprint, door
  side, storeys); placement on lots; the generated mod; the two scenes in game.

ST-1 and ST-3a run in parallel (the format above is the contract); ST-2 after ST-1, ST-3b after ST-3a.

## Milestone 2 plan: arm B in the 10x world (2026-11-24, draft for the owner)

Research: `docs/research/settlements-m2-placement-2026-11-24.md` (this repo) and Scrib
`docs/research/settlements-m2-scrib-side-2026-11-24.md`. Goal: Balmora 2.0 (arm B, first culture `medina`) seen in
`view "world:<guide>"` with Balmora's own Hlaalu buildings on the lots, the ground graded to the plan, the streets
paved, the old Balmora gone.

**Decisions (from the research and the industry; not owner questions):**
- **Buildings on lots = catalogue best-fit in the lot's frontage frame** (CityEngine rule inserts, UE5 City Sample,
  cityforge stamps): Magna Urbis's own per-lot building footprint (`building_sites::requests` `preview_parts`,
  `parcels/occupation.rs:2176`) is the buildable domain, the door faces `accesses[0]` (the widest public access),
  a candidate is rejected when its hull leaves the domain, overlaps another or its pad step exceeds a tunable.
  Whole prefabs only, never cut meshes. The kit is Balmora's own: ~48 buildings from 30 models (fronts 8-24 m,
  depths 8-16 m), walls, stairs, bridges, canal pieces, lamps (Scrib note §4).
- **Files only, generic both ways:** Scrib writes `kit.json` (per model: id, hull, oriented box, height, base
  offset, door anchors, kind tags); the tool writes `placements.json` (kit id, world x/y/z, yaw, lot id) and
  `grade.f32` + `grade_w.u8` + `grade.json` (planned heights and blend weight, ~1 m grid over the town plus its
  blend margin) and `ground.u8` (street/yard classes). Formats in the placement note §5, versioned like `site.json`.
- **Into the game as a mod:** Scrib's importer turns `placements.json` into a native `.ron` mod of
  `sf.placement_group` records (existing framework kind, no reader yet), read into `WorldgenContent.extra`
  (`mw_exterior_bevy/src/worldgen.rs:105`, the seam the flora scatter uses); the planned X6 baked 10x worldspace
  reads the same records later, so the producer never changes. Collision comes with the normal static path.
- **Terrain and paving through the stamp mechanism:** a derived guide folder (`x10s` stays the A/B base) drops the
  three Balmora anchors (`anchors.json` 2, 200, 513), zeroes stamps 2 and 188 (indices kept) and appends a town stamp
  from `grade.f32`/`grade_w.u8` with its own ground-layer grid (streets at 1 m, `sf_worldgen/src/layers.rs:50`:
  weight > 0.5 takes the stamp's layers). One terrain provider feeds render, physics and groundcover. Vertical faces
  (retaining walls, kerbs) are meshes, not heights (0.5 m finest height sample). Paving: Balmora's own textures,
  cobblestone on main streets, dirt road on lanes (the road LTEX ids are already in the `x10s` palette).
- **No doors in the first look** (facades); the door policy is an owner question for later, with old Balmora's
  quests and NPCs.

**Slices (one deliverable each, in order):**
1. **S1 (Scrib) kit export**: `bevyrrowind worldgen-kit` writes `kit.json` for an anchor's human-made models (hull
   and box from the baked model info, base offset, door anchors from the vanilla door references beside each model)
   and prints a size histogram. Test: fixture model round trip.
   *As built 2026-11-24:* `mw_worldgen/src/kit.rs` + `bevyrrowind/data/worldgen-kit.toml` (ordered path rules ->
   kind), tunables `kit.door_radius_m` 1.5, `kit.door_agree_*`, `kit.bin_m`. Balmora: 124 models from 643
   references in 2.4 s; buildings 35 models / 54 references, fronts mostly 6-16 m (max 26-28), depths 6-14 m;
   wall 10, arch 5, stairs 8, bridge 5, canal 4, lamp 5, prop 52; 32 of 35 building models have doors. `frame` is
   the model's own (door side from `facing_deg`); `tags` carry `base:<record id>`. Found: Hlaalu meshes sit centred
   on their pivot (`base_offset_m` ~ -h/2), doors differ between uses of a model (upper-floor doors ~3.7 m above the
   street door, doorless uses, a roof trapdoor), and a model keeps its first reference's doors.
   *S1b + S1c as built 2026-11-24:* door facing = outward normal of the hull edge the door stands on (corner tie
   broken by the mesh normal, `kit.door_corner_m`); doors from the reference with most doors, street door first,
   trapdoors dropped (7); `fb_01`, `striderport_01`, `velothi_temple_02` -> `prop`: 53 of 53 building doors match
   their edge. `ground_z_m` (median over references of the source terrain at the street door's outside point,
   `kit.pad_probe_m` 1 m, or the hull centroid; model frame) and `foundation_depth_m` = `ground_z_m - base_offset_m`:
   buildings -6.96..-0.33 m; the ground sits ~0.77 m (median) below the street door's sill (a doorstep in the
   mesh), so placement seats on `ground_z_m`, not the sill; b_13, b_15, b_21 have only a raised door 4.4-5.2 m up
   (stairs, later). Output `target/worldgen/kit/balmora-ground` until PL-2b's reader takes the field.
   **S1b (queued, now done):** per model, the vanilla ground height in the model frame (median over references of the source
   terrain under the hull minus the reference z) as `ground_z_m`; doors from the reference with most doors, street
   door (lowest sill) first; trapdoors (vertical facing) dropped. **PL-2b:** pads use `ground_z_m` (instance z = pad
   z - ground_z_m), the street door orients the yaw.
2. **PL-0 (tool) `lots.json`**: `settle` calls `requests` and writes per lot shape, front, access, role, floors,
   `preview_parts`, with a timing line; falls back to parcel data if a request fails. Test: fixture, every lot
   listed, same seed same file. S1 and PL-0 run in parallel: they settle the two measured unknowns (Hlaalu sizes,
   `requests` cost).
   *As built 2026-11-24:* `crates/settle/src/lots.rs`; `lots.json` per culture (world frame; per lot id, polygon +
   holes, use, front, axis, access = widest public, role/function/wealth, floors, `ground_z_m` = the site's own
   height at the lot's interior point, `preview_parts` as `{label, polygon, holes}`); fallback to parcels with an
   `error` header field; `requests_ms`, `lots_with_preview` in `stats.json` only (lots.json stays byte-identical
   per seed). Arm B: medina 773 lots, 507 with a footprint (no request for `open.garden`: 260 of 266 empties),
   median 13.0 x 18.0 m per lot, 9.2 x 9.1 m per part; `requests` ~40 ms. Footprints may stand up to 3 cm outside
   their lot (test tolerance 0.05 m).
3. **PL-1 (tool) formats**: serde types and readers for `kit.json` / `placements.json` / `grade.json`, round-trip
   tests, the contract section here.
4. **PL-2 (tool) fit**: placement by best-fit, no merging, unfilled lots left empty; stats (filled share by role).
   Test: fixture block, doors face the street, no hull overlap, deterministic.
   *As built 2026-11-24:* `crates/settle/src/fit.rs`, recipe `[fit]` (10 tunables), `--kit` flag (the kit path is
   machine-specific, so not in the recipe). Each MU part fitted on its own with every `building`; street normal from
   access, then front, then axis, pointing away from the lot centroid; empty reason = furthest test passed (inside <
   overlap < pad_step). Arm B: medina 507 lots tried, 243 filled (233 empty: no model fits inside the part, 31: pad
   step), 298 instances, 29 models, 103 ms. Exposed kit bugs (S1b: door facing, three doorless landmarks typed
   `building`); ~35 % of tried lots are narrower than every model, for PL-3's merging.
5. **S2 (Scrib) guide derive**: `bevyrrowind worldgen-derive` (drop anchors by name, zero their stamps, append a
   stamp + layer grid from files). Test: stamp indices stable, a zero-weight stamp changes nothing.
   *As built 2026-11-24:* `mw_worldgen/src/derive.rs`, `sf_worldgen::stamps` `StampSet::zero_weights`/`append`
   (no-overlap check both ways, a refused append changes nothing; sample budget `anchor.stamp_samples_max`).
   `bevyrrowind worldgen-derive --guide <dir> --out <dir> --drop-anchor <name> --add-stamp <dir>` (no game data
   needed); `derive.json` records the source hashes and what changed. Real: `target/worldgen/x10s-b2` = x10s minus
   "Balmora", "Balmora 2", "Balmora 3" (stamps 2 and 188 zeroed), 0.2 s. An appended stamp without a layer grid
   falls back to the global grid (`layers.rs:438-442`). Rewritten JSON floats may differ by one ulp (serde_json
   without `float_roundtrip`; ~1e-13 m, harmless).
6. **S3 (Scrib) placement importer + reader**: `placements.json` -> `.ron` mod -> `WorldgenContent.extra`. Test: a
   two-building mod appears at its positions.
   *S3a + S3b as built 2026-11-24:* `bevyrrowind worldgen-town --placements --kit --out` (`mw_worldgen/src/town.rs`)
   writes a native mod (`mod.toml`, `records/town.ron`: one `sf.placement_group` `worldgen.town.<name>` in
   `@exterior`, bases from the kit's `base:` tags, flora's coordinate conversion, kit hash checked). **A guide carries
   its own mods** (`<guide>/mods/<name>/`): a town fits only the terrain it was planned on, and must never appear in
   a vanilla run. `view "world:<guide>"` (`apps/bevyrrowind/src/world.rs` `guide_towns()`) applies their records into
   the content db after the profile loads and turns each exterior group into an extra entry `town:<id>`;
   `sf.placement_group` is now `register_serde`. Gap: the guide's mods don't join the load order (assets not
   mounted, profile fingerprint unchanged): a later slice. Medina (old plan): 275 statics, all resolved.
   *Re-plan on true heights (2026-11-24, `recipes/balmora2-x10s-b2v.toml`, site `balmora-x10s-b2v`):* medina 4 lots,
   european-organic 52, korean 340: the terrace risers had no way up (Scrib `site-prep.md` "Ramps", "Stairs").
   **Finding 2026-11-24 (`docs/research/magna-urbis-slopes-2026-11-24.md`): Magna Urbis plans steep sites itself.**
   It never edits terrain, but routes stair streets (`maxStairGrade` <= 1.0), follows contours and terraces wards
   when `tissue = "terraces"` (or a stair culture), and gives each building a footing (plinth, terrace with a
   retaining wall height, stilts) with a base height. Our recipes had all of it off. On the **true** arm B terrain
   (site `balmora-x10s`, relief 1.0, `[magna_urbis.layout] tissue = "terraces"`, `maxStairGrade = 1.0`,
   `maxLaneGrade = 0.4`): medina 923 lots (507 with a footprint), al-andalus 890, italian-comune 951,
   latin-american-hillside 995; slope rejections 7-11; a full town on both banks with its bridge. So reshaping the
   valley is not needed for planning (owner to decide what stays of SP-1).
7. **PL-3 (tool) tissue**: the owner's answer to look question 1 (merge + filler walls, or detached).
   *PL-3a as built 2026-11-24:* `crates/settle/src/merge.rs` `fit_and_merge` after the first pass (`fit.rs`
   `Pass::fit_domain` shared, sees every hull placed so far); recipe `[merge]` (7 tunables: max_lots 3, max_front_m
   24, ...; no table = no merging). Frontage neighbours: shared boundary, gap measured on the lot's street face,
   offset at the nearer frontage end (winding lanes), angle test. Merged domain = union of the members' lots cut to
   their footprints' depth band along the street (keeps MU's building line). Arm B medina: 213 -> 271 built lots,
   29 groups of 2; still empty: no neighbour 139 (corner/back lots), merge limit 34, pad step 30 + 22, inside 11.
   *PL-3b as built 2026-11-24:* `crates/settle/src/filler.rs` (`fit_merge_fill`); recipe `[filler]` (wall and
   arch model ids as data, Hlaalu wall and gate pieces; requires `[merge]`, whose frontage runs it reuses); pad z in
   one place (`fit::pad()`). Walls: whole pieces end to end on every uncovered lot street face (gardens included);
   arches: lane mouths = gaps between the facing ends of two runs on one street line. Arm B medina: frontage coverage
   0.269 -> 0.810, 1381 walls, 13 arches; skipped: pad step 398 (sloped faces), gap too short 140, lane mouth too
   narrow 48 / too wide 7. Open: the taller `wall_up` pieces are never picked (same lengths as `wall_01`): stepped
   walls on slopes need a slope rule (later slice).
8. **PL-4 (tool) grading**: street profiles at Magna Urbis's grades (street 0.12, lane 0.18, stairs 0.65), pads at
   building base, blend to untouched ground over at least cut / tan(talus); cut/fill reported. Needs look question 2.
9. **S4 (Scrib) scene**: the derived guide with the town stamp and mod, a `view` scene at the new Odai crossing,
   registered as an owner check. First look.
10. Later: retaining walls and stairs as features (PL-5), GC-M4 on generated ground (it reads the bake's terrain
    today), doors and door links (WG-A3b), lamps by street, more cultures.

**Owner answers (2026-11-24):**
1. **Medina = continuous street wall:** narrow lots merged until a Hlaalu house fits, gaps between houses closed
   with Hlaalu walls and arches (PL-3). (Rejected: detached Balmora-like houses; another culture.)
2. **Reshape the valley before planning:** the Odai reach is reshaped into a wider terraced valley as part of the
   generated terrain, then the town is planned on true heights (`relief_scale` 1.0), so it fits by construction and
   PL-4 is only pads and street profiles. (Rejected: cutting the plan into the gorge; keeping the cliffs.)
   This adds, before PL-2's real runs: **SP-R** (research + spec, no code: the site-preparation profile: channel
   kept, terrace widths and heights, risers as steep banks or walls, blend to the gorge beyond; industry: hill-town
   terracing, site prep in worldgen pipelines) and **SP-1** (Scrib: the site-prep stamp in the derived guide, S2's
   tool), then ST-2's export from the derived guide and a `settle` run at `relief_scale` 1.0. S1, PL-0, PL-1 and
   PL-2 on fixtures are unaffected and go first.
3. **SP-R found (Scrib `docs/research/settlements-site-prep-2026-11-24.md`):** south of the centre both banks are a
   flat plain (+23..28 m, slope < 0.07) cut by a 140-165 m wide trench whose ~29 m wall is the steep ground; cliffs
   and hills only north. SP-1 = a generic `valley_terrace` stamp producer (1 m, treads down to a quay, risers,
   weight 0 where wet, blend to the untouched ground), no runtime code. Owner, same day: **keep the river at full
   width, crossed by one long multi-arch bridge** of repeated Hlaalu bridge segments with piers (settle's crossing
   already accepts spans to 400 m; the bridge assembly is a later slice with the kit); **stone quay** at the water
   (Hlaalu canal wall, stairs down); **risers: Hlaalu retaining walls with stairs inside the town, earth banks
   where the terraces run out past the buildings** (SP-1 flags risers by town extent; PL-5 builds the walls).

## Open questions (owner, player-facing; not needed for Milestone 1)

- Interiors of generated buildings: none, locked doors, or generated interiors.
- NPCs and services in new quarters.
