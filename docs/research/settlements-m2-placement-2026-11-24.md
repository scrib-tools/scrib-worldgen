# Settlements Milestone 2: prefab placement, grading, contract (research, 2026-11-24)

Scope: arm B ("Balmora 2.0", culture `medina`, `recipes/balmora-x10s.toml`), relief scaled 0.25. Research only; no code run.
Marks: **V** verified (file:line in the pinned Magna Urbis checkout `MU/...` = `rust/crates/core/src/urban/...`, or URL), **U** unverified.
Prior notes (Bevyrrowind `docs/research/settlements-*-2026-11-23.md`) are cited as "prior note"; their facts are second-hand here.

## 0. Top findings

1. MU already computes a building footprint inside every lot: `preview` / `preview_parts` of `building_sites::requests` (V `MU/building_sites.rs:1371-1372, 1630`), made by `parcels/occupation.rs:2176` `preview_parts`: whole rectangles in the frame of the main frontage, fitted inside the lot by an exact slice test, or the lot band itself when its sides are within 15 degrees of square (V `occupation.rs:1-12`). We consume it as the **buildable domain**; we do not invent a footprint.
2. `requests` needs only what `settle` already has: layouts, partitions, the `Surface`, a per-cell `water_level: &[f32]`, optional culture (V `building_sites.rs:1671-1680, 994`). Cost is **U** (never run); it is parallel per site (`par::range`, V `:1766`) and prints its own timing with env `URBAN_PROFILE` (V `:1688-1694`).
3. **Scale mismatch is the main risk.** Medina lots are narrow and deep (ring: frontage 5.5-11 m, depth 30-50 m, buildDepth 9-13 m, so a footprint of about 50-140 m2; core: 4.6-6 m x 5-8 m shop rows; V catalogue `medina/core`, `medina/ring`). Hlaalu buildings are freestanding boxes whose size we have not measured (**U**; the kit catalogue slice measures it). Most lots will need **merging** before a prefab fits.
4. **Grading is large with `relief_scale` 0.25.** The plan sits at 25 % of the height above water, so under the town the planned ground is up to 75 % lower than the game's: a 23-25 m bank (recipe note, V `recipes/balmora-x10s.toml`) becomes about 6 m, i.e. an 17-19 m cut at the town edge. That is a cut-and-daylight earthwork, not a stamp blend; the blend band must be at least `cut / tan(talus)` wide. This needs the owner's eyes (look question 2).
5. MU does no terrain grading itself (it has only grade limits: street 0.12, lane 0.18, stair 0.65, V `MU/config.rs:342-344`, and per-building footing hints `plinth|terrace|stilts`, V `massing.rs:360-417`). Grading, ground classes and placement are ours.

## 1. What Magna Urbis gives per lot

**Parcel** (V `MU/types.rs:338-362`): `id`, `parent_ids`, `provenance`, `family`, `reason`, `shape` (polygon with holes = the lot), `envelope: Option<Shape>` (the admission witness: what a house provably fits), `block`, `use` (`use_kind`: plot, compound, garden, reserve...), `front: Option<[Point;2]>` (frontage segment), `axis: Point` (frontage direction), `rank` (street rank), `pressure` (0-1), `contacts: Vec<ParcelContact{a,b,street,rank,length,kind}>` (every street edge touched; `street` indexes `layout.streets`; kind public/private/collective/unserved/reserved), `access{kind, street, front_length}`. Coordinates are MU local metres, y south (our wrapper keeps that plus a `frame`; V spec "As built").

**Allocation** (V `MU/programme/model.rs:121-153`, in `partition.programme.allocations`, join by `parcel_id`): `function` (one of six houses functions), `role` (rich: e.g. `worship.parish`, `commerce.inn`, `dwelling...`), `landmark`, `households`, `residents`, `culture`, `architecture_pack`, `district_role`, `wealth` (0-1), `capacity/unit`, `housing_floor_need`. `generate_parcels` already runs allocation (V `MU/pipeline/parcels.rs:127-139`, chain `generate`, `allocate_cadastre`, `consolidate_cadastre`, `finish_cadastre`, `face_quays`), so `partition.programme` is filled today; whether `settle`'s `layout.json` keeps it is **U** (it serialises the partition; check).

**Building request** (one JSON per building, V `MU/building_sites.rs:1628-1637`): `id` (`<parcel>/building`), `parcel_id`, `placement_hint`, `preview` (footprint polygons), `preview_parts` (label per part: "building", court, prayer space...), `programme` (`floors` [min,max] or null, `composition` e.g. courtyard/detached, `district_role`, `wealth`, `built_area_m2`, `massing`, `frontage_continuity`), `occupation`, `feasibility`, `cadastral`/`buildable`/`roof_domain` (the lot), `reserves` (holes), `accesses` (`{id, segment, kind public|private|collective, width_m}`, sorted widest first, so the first is the main frontage; V `:23-40`), `party_boundaries` (shared lot edges, only when the culture's `party_walls` is true), `culture{culture, pack, mode}`, `function`, `seed`, `terrain{grid, origin, ground_z_m, gradient, water_z_m, max_foundation_step_m}`. The module is independent of the houses engine (V `building_sites.rs:1-2`).
- `party_walls` is true when the pack layout is strip, courtyard, siheyuan or pueblo (V `MU/culture.rs:1143-1146`); medina maps to `courtyard`, pack `AF-MAG` (V `culture.rs:64`), so medina requests carry party boundaries.
- Slope: footing `{way plinth|terrace|stilts, base_z_m, low_z_m, drop_m}` per preview piece when the drop is >= 1.0 m (V `massing.rs:393, 398-417`); a home is turned on its lot to follow contours (`hillside.rs` header, V); `max_foundation_step_m` default 0.75 m, relaxed up to 6 m (V `feasibility.rs:22-29`). These read MU's own heights, i.e. the 0.25 plan relief, not the game's (see section 4).
- Door side: not explicit (V prior note); take `accesses[0].segment` (widest public) else `parcel.front`; the door faces away from the lot into that segment.
- Footprint kinds MU picks: Open, Row, Detached, Farm, Court, Insula, Pavilions, Hanok, Yashiki, Round, Long, Hall, Warehouse (V `occupation.rs:22-37`). A court keeps at least the wing width on every side; a mass that does not fit is dropped, not clipped (V `occupation.rs:6-12`). So `preview` for a medina courtyard lot is a ring or L (with holes) that a rectangular prefab cannot reproduce; the prefab replaces the building and the court becomes yard.

**Can `settle` call `requests`?** Yes: it holds `layouts`, `partitions`, `prepared.grid.surface()` (V `crates/settle/src/run.rs:130-137`). Extra: `water_level` per surface cell (only read where `surface.wet(center)`, V `building_sites.rs:994`; a constant `water_level_m` array of surface length suffices, **U**), and architecture culture `None`. The admission table is `include_str!`'d from `houses/data/urban-admission.json` (V `feasibility.rs:182`), so no new dependency, but it is compiled into the `core` crate and `settle` already compiles `core` (the file is in the checkout). Requests can be large (page limit 24 MiB, V `:795`); we need only 6 fields, so write a thin `lots.json` and drop the rest. Cost: **U**, expect seconds for ~1000 lots; measure once with `URBAN_PROFILE=1` before design lock. Risk: `Err` on any one request fails the whole call (first error wins, V `:1780`), so the call needs a fallback path to parcel-only data.

**Other MU stages that size a building**: `massing.rs` and `display.rs` (volumes; we skip), `forms/` (houses kit, not needed), `farmsteads.rs` (farm lots), `activities.rs` (craft yards: `retained_yard`, V `building_sites.rs:1362-1366`, lays several buildings on one lot, so a lot can carry more than one preview part). Consume `preview_parts` not `preview` alone.

**What `medina` sets** (V `MU/catalogue.json`, culture `medina`, presets `medina/core|ring|faubourg|village|hamlet`): pattern organic, walled (`fortification wall`, organic enclosure), nucleus a mosque (rect 3200-9000 m2, compound), `deadEndRatio` 0.9 (many dead-end lanes), `widthByRank` core `[5,3.8,2.8,2.4,2.2]` m, ring `[6,4.4,3.2,2.4,2.2]` m. Core: `plotOp burgage` (shop rows), frontage 4.6-6 m, plot depth 5-8 m, coverage 0.93-0.98, setback 0-0.3, sideGap 0, storeys 1 (`souk-shop`). Ring/faubourg/village: `plotOp courtyard`, frontage middle 6.5-11 m (edge 8-14, village 18-40), plot depth middle 30-50 m, buildDepth 9-13 m, coverage 0.88-0.95 (faubourg 0.6-0.75), houseArea 90-320 m2, setback 0-0.8, sideGap 0-1.2, storeys 1-3, flat roof, mud. Blocks middle 7000-22000 m2.

## 2. Industry technique: prefabs on generated lots

Canonical name: **lot -> setback -> buildable rectangle -> rule table inserts an asset** (CityEngine CGA; UE5 City Sample "Rules Processor"), with a fallback for what does not fit.

- **CityEngine** (V https://doc.arcgis.com/en/cityengine/2023.0/cga/cga-inner-rectangle.htm): `innerRectangle` finds the largest rectangle fully inside each face, sides parallel to the scope axes (`alignScopeToAxes`), with `shape` and `remainder` branches; the remainder is the filler. `splitAndSetbackPerimeter` (V https://doc.arcgis.com/en/cityengine/2023.0/cga/cga-split-and-setback-perimeter.htm) splits the lot perimeter into sections and sets each back by its own depth (front yard vs sides). The documented weak point is irregular lots, where the rectangle is aligned to scope axes, not to the street (Esri community thread, no answer in the excerpt: https://community.esri.com/t5/arcgis-cityengine-questions/cga-strategies-for-dealing-with-irregular-shaped/m-p/1686741). Assets inserted by CGA `i("asset.obj")`: **U** (from memory; the search returned only the operations above).
- **UE5 City Sample / Matrix Awakens**: Houdini layout makes roads, blocks, lots and massing; points go to a Rules Processor that places kit pieces; later on PCG (prior note, V abstracts; pages not re-read here).
- **Plot-with-frontage model** in a recent procedural-city write-up: blocks subdivided into oriented plots (a rectangle with a frontage direction), buildings fitted to plot orientation, those that do not fit "shrunk or dropped", ground-contact audit (V https://forums.unrealengine.com/t/mythralen-ue5-procedural-rpg-world-work-in-progress/2747626 via search snippet only; hobby project, weak source).
- **cityforge** (Procedural Tamriel, ideas only; prior note, second-hand): whole-building stamps with door and hull, hull-overlap as the only hard spacing rule, door-step priors, a `flatten_pad` request when a lot needs grading, kit catalogue accepted visually before layout runs. A web search for it found nothing public (V, searched 2026-11-24); keep it as ideas from the prior note.
- **Daggerfall, Bethesda**: hand-authored blocks or placed kits; no lot logic (prior note).
- Papers: Parish and Mueller 2001, Mueller et al. 2006 (CGA), Weber 2009 (prior note). Largest inscribed rectangle in a polygon: classic computational geometry (Alt et al. 1995 for axis-parallel; **U** exact citation). We need an *oriented* rectangle (frontage frame), which is the easier case: rotate into the frontage frame and slide.

**Method we should use (generic, our code):**
1. Per lot, frame = `accesses[0].segment` (front a->b; u = unit(b-a); n = normal pointing out of the lot toward the street). Domain D = union of `preview_parts` labelled building (MU's own footprint, already setback and court-aware), else the lot shape shrunk by a setback tunable.
2. Candidates = kit entries whose `kind` and `roles` match the allocation (`role`, `function`, `wealth`, `floors`), yaw so the kit's door normal equals n (kit door facing from the catalogue; `yaw_step_deg` tunable for kits that only rotate by quarter turns).
3. Fit test: kit hull (oriented rectangle or convex polygon) slid along u in steps of `slide_step_m` and offset along n by 0 .. `max_setback_m` from the frontage line; accept if hull minus D has area under `eps`. Same exact slice idea MU uses (V `occupation.rs:6-9`).
4. Score = covered share of D, minus `frontage_gap` (|kit width - frontage|), plus a role/wealth match term; pick the top candidate, or a seeded weighted draw among the top k for variety (deterministic: hash of lot id and recipe seed). Cost: lots x kit x yaw x slides ~ 10^6 polygon tests for 1000 lots x 50 kits; fast (**U**).
5. Irregular lots: the frontage frame handles skew; if no candidate fits, try (a) `merge` with an adjacent lot on the same street and block (shared edge, both fronts within `merge_angle_deg`), (b) a smaller kit class, (c) filler. Greedy merge in a fixed order (by lot id), at most `merge_max_lots` lots.
6. Slope: reject a candidate when the height range under the hull on the planned surface exceeds `max_pad_step_m`; otherwise the grading stage gives it a pad (section 4). Footing hint from MU (plinth/terrace/stilts) maps to kit tags only if the kit has such variants (later).
7. **Fillers (lots no prefab fits)**: priority list from the kit: low wall along the frontage (kind `wall`), yard props, stall (market lots), well/cistern, or empty with a `status: "empty"` record. Never leave a hole silently; stats count each status (filled / merged / filler / empty) with built-area share.

## 3. Medina fabric with a freestanding-box kit

Problem: MU's medina is party-wall courtyard houses on 6-11 m fronts and dead-end lanes (V section 1); the kit is freestanding rectangles (U for Hlaalu sizes). Options:

A. **Merge lots, keep medina streets** (recommended default). Merge 2-3 adjacent narrow lots until a prefab fits; the prefab sits on the street line (setback near 0, as the medina data says, V sideGap 0-1.2); remaining gap on each side is closed by a **filler wall or arch** piece of the same kit (kind `wall`/`arch`) so the street still reads as a continuous wall, as a medina does. Reuses all of MU; costs a merge step and needs filler pieces.
B. **Choose another MU culture for the tissue** (Hlaalu is "near italian-comune or venetian-lagoon", prior note, guess). Detached-with-gaps cultures fit freestanding boxes with no merging; but arm B names `medina` and the owner rates cultures by elimination, so run them as extra arms, not instead.
C. **Edit the medina lot cut via recipe** (`magna_urbis.parcels.params` override of frontage/plotDepth/sideGap; `ParcelConfig.params: Value`, V `MU/config.rs:534`; whether it overrides the preset per key is **U**) so lots come out kit-sized. Generic and cheap to try; changes the street/lot look of the culture, so it is a data decision.
D. **Courtyard compound from several prefabs** (3-4 boxes around a walled yard on one merged lot). Fits the medina look best but needs a compound rule; later.

Recommendation: A first (tool-side merge + filler), with C as a recipe switch and B as extra arms. Tissue-fit stats in `stats.json` (lots filled by one prefab, merged, filler, empty; prefab hull coverage of MU's `built_area_m2`) let the owner see which culture suits the kit. The look of "freestanding box + wall gaps vs party wall" is a look question.

## 4. Grading in the tool

**Standard technique** (V where linked): spline/road alignment tools build a flat margin along the spline, then a falloff band to untouched ground (BeamNG master spline tool, falloff 3-4: https://documentation.beamng.com/world_editor/tools/master_spline_tool/advanced_usage/ ; UE "Deform Landscape to Splines", known artifacts and extra segments needed: https://forums.unrealengine.com/t/deform-landscape-to-splines-doesnt-always-deform-landscape/307507.md ). CityEngine terrain alignment, Houdini heightfield flatten, Cities: Skylines: **U** (searches returned nothing; they use the same pattern: mask, target height, blend, plus cut/fill limits). cityforge (second-hand): pads, road grade max 0.125, blend margins 768-2048 game units, cut/fill caps 1024 game units, then repaint ground textures and recompute normals (prior note).

**Our procedure (generic, in the tool):**
1. **Street profiles** on the street graph (nodes shared by streets, so junction heights agree; whether MU street paths end exactly on shared nodes is **U**): sample the ground height along each centreline, then solve a grade-limited fit: minimise sum |h - h_gen| subject to |dh/ds| <= grade(rank) (street 0.12, lane 0.18, stair 0.65, MU's own limits, V `config.rs:342-344`); a forward-backward Lipschitz clamp is the cheap version, a small LP/DP the exact one (**U** which is needed). Rank 0-1 smoother than lanes. Tunables: grade per rank, max vertical curvature (**Guess**).
2. **Carriageway**: full width at the profile height (cross-slope 0 first; 2 % drainage is a later look item, **U**), then a **verge** band (`verge_m`) at the same height, then the blend.
3. **Lot pads**: per placed prefab, pad = hull + `pad_margin_m`, height `z_pad` = median of the planned ground under the hull, clamped to within `max_door_step_m` of the street height at the door (door-step priors exist in cityforge, prior note), cut/fill capped at `max_cut_m` / `max_fill_m` (cityforge caps are 1024 game units, about 14.6 m only if 70 units per metre, **U**). Lot remainder: blend from the pad to the street/neighbour heights across the lot (smootherstep) or a gentle plane.
4. **Daylighting / blend to untouched ground**: from the union of painted areas, distance d; weight `w = 1 - smootherstep(d / band)`; blend width = max(`min_blend_m`, cut / tan(`talus_deg`)) per location, so a deep cut slopes back at the talus angle (civil road practice uses roughly 1:2 slopes, **U**).
5. **Where the grade cannot be met** (pad needs more than the caps, or street grade would exceed the stair limit): emit a **retaining wall** polyline (top and base height) or a **stair** feature along the street; the placement file lists them as features Scrib instantiates from kit kinds `retaining_wall`, `stairs`.
6. **Relief scale caveat.** Plan heights are `water + 0.25 x (h - water)` (V recipe). If the stamp uses that as `h_plan`, the town is a bowl 75 % shallower than the game; the cut at a 23-25 m bank is 17-19 m, the blend band at talus 1:1 is 18-19 m, at 1:2 about 36-38 m, and the gorge cliffs (V banks 23-25 m, terraces between cliff bands, recipe note) are bulldozed. Options: (i) accept it (a widened valley floor; the plan is consistent with the ground), (ii) plan on real relief with a higher street grade (breaks MU: no quarter street routes, V spec ST-3d/3e), (iii) plan at 0.25 and grade only streets and pads with step/wall features at the original ground (town on terraces, ground between kept). (iii) is the least destructive and matches what shipped games do (streets and pads cut into slopes with retaining walls), but cannot keep the plan heights everywhere. A look question.

**Output proposal** (files next to `layout.json`):
- `grade.f32`: little-endian f32, row-major, metres absolute (same vertical datum as `site.json` heights), `w*h`, row 0 north (same convention as `site.json`).
- `grade_w.u8`: blend weight 0-255 (= 0..1), same grid. Scrib applies `h = mix(h_gen, h_plan, w)` per sample (spec stamp format v1 with a smootherstep band exists in Scrib, prior note; exact fields **U**, check `superfargoth/docs/specs` before fixing).
- `grade.json`: `format: 1`, `origin_m`, `cell_m`, `dims`, `frame` (world metres, x east, y north), `blake3` per file, `max_cut_m`, `max_fill_m`, `cut_m3`, `fill_m3`, `daylight_band_m`, `source` (recipe name, seed).
- Resolution: the grid covers only town land plus the blend margin, at `grade_cell_m` (tunable, default **1 m, Guess**: 2.2 m lanes need 2 cells across). A 1 km square at 1 m is 4 MB for `grade.f32`; fine. Use the site's frame so Scrib does no resampling when `grade_cell_m` equals a multiple of `site.cell_m` (site spacing for arm B **U**; check `site.json`).
- `ground.u8` (same grid): class raster, 0 = none, others from a legend in `grade.json` (`street_rank0..4`, `plaza`, `yard`, `verge`); Scrib maps classes to ground textures by its own table (generic: no texture names in the tool).

## 5. Contract extension (files only, versioned like `site.json` format 1)

**(a) Kit catalogue**, written by Scrib, `kit.json` format 1 (+ `kit/*.hull.json` not needed; inline):
```
{ "format": 1, "name": "...", "producer": "...", "units": "m", "frame": "local x right, y front (door side), z up; pivot at model origin",
  "models": [ {
    "id": "string (stable; Scrib's own id for the model)",
    "kind": "building|wall|arch|stall|yard|prop|stairs|retaining_wall",
    "tags": ["free text, e.g. wealth:low, floors:2"],
    "roles": ["MU role or function strings this model may serve, e.g. dwelling, commerce.inn"],
    "footprint": [[x,y],...],            // convex hull of the mesh on the ground, local metres
    "obb": { "centre": [x,y], "half": [hx,hy], "yaw_deg": 0 },   // optional oriented rectangle of the hull
    "doors": [ { "at": [x,y], "facing_deg": 90, "width_m": 1.4, "sill_z_m": 0.2 } ],
    "height_m": 7.5, "floors": 2,
    "base_offset_m": -0.4,               // lowest mesh point relative to pivot z; pad sits at the sill
    "foundation_depth_m": 0.6,           // mesh below the sill that may bury into ground
    "yaw_step_deg": 0,                   // 0 = any yaw, 90 = quarter turns only
    "weight": 1.0,
    "blake3": "hash of the source mesh"
  } ] }
```
Facing convention: `facing_deg` is the door normal in the local frame, 0 = +x, counter-clockwise; the tool rotates so that normal points at the street. Footprint as hull (not OBB only) because cityforge's rule is hull overlap (prior note); the OBB is a fast pre-filter. Door list rather than one door because inns and shops have two (**U** for Morrowind data).

**(b) Placements**, written by the tool, `placements.json` format 1 (world metres, x east, y north, z up; yaw counter-clockwise about +z viewed from above; world = pos + R(yaw) x local; round-trip test with a fixture required):
```
{ "format": 1, "name", "producer", "recipe": "...", "seed": "...", "kit_blake3": "...", "origin_m": [x,y],
  "lots": [ { "id", "polygon": [[x,y],...], "status": "filled|merged|filler|empty", "role", "pad_z_m", "front": [[x,y],[x,y]], "merged_from": ["id"] } ],
  "instances": [ { "id", "kit": "model id", "lot": "lot id", "pos": [x,y,z], "yaw_deg", "role": "building|filler|wall|...", "door": [x,y] } ],
  "streets": [ { "id", "rank", "role", "polyline": [[x,y,z],...], "widths_m": [w,...], "surface": "class name from ground legend" } ],
  "features": [ { "kind": "retaining_wall|stairs", "polyline": [[x,y,z]], "base_z_m", "top_z_m" } ],
  "grade": "grade.json" }
```
Notes: lot ids are Magna Urbis `Parcel.id` (stable per seed); `streets` carry planned z so Scrib can place lamps or paint roads without resampling; `widths_m` per vertex as MU's `Street.widths` (V `MU/types.rs:8-13`). Scrib treats `placements.json` as a mod (generated mod), converting `kit` ids back to its own models, so the tool never names Morrowind. `layout.json` stays MU's local frame (y south) plus `frame`; `placements.json` is always world frame (so only one conversion lives in the tool).

## 6. Recommended slice order (one deliverable each)

Tool side (`scrib-worldgen`): 
1. **PL-0 `lots.json`**: call `requests` in `settle`, write per lot {id, shape, front, accesses[0], role, function, wealth, floors, preview_parts, ground_z, `built_area_m2`} plus a timing line; test on the synthetic fixture (every lot has an entry, deterministic). Settles the cost question (U in section 1).
2. **PL-1 kit + placement formats** (serde types, readers, round-trip tests, a synthetic kit of 6 boxes, `docs/specs/settlements.md` contract section).
3. **PL-2 fit**: frontage-frame fit, no merging, filler = empty; stats: filled share by status; test: boxes fit a fixture block, door faces the street, no hull overlap, same seed same output.
4. **PL-3 merge + filler walls**; stats of tissue fit per culture.
5. **PL-4 street profiles + `grade` files** (streets only, then pads, then daylighting); test: grade limit holds, cut/fill reported, flat terrain gives w=0 change.
6. **PL-5 ground classes + retaining walls/stairs features**.

Scrib side (GPL): 
S1 (parallel with PL-1) **kit exporter** for Hlaalu: hull, doors, sill, height, base offset from the meshes; also prints footprint size histogram (settles the Hlaalu size U). S2 **terrain stamp from `grade.f32`/`grade_w.u8`** and ground classes to textures. S3 **generated-mod loader** for `placements.json` (instances, features, lamps by street). S4 the arm B scene in the 10x world, registered as an owner check.

Order: S1 and PL-0 first (they answer the two measured unknowns); PL-1 follows; the owner's look questions below should be answered before PL-4.

## 7. Look questions for the owner

1. Medina street wall with Hlaalu boxes: **merge lots + filler walls between boxes** (continuous street wall), or **detached boxes with gaps** (Balmora-like, but not medina), or another culture for the Hlaalu kit? (section 3)
2. Grading amount at `relief_scale` 0.25: **widen the gorge into a terraced valley** (town as planned, big cuts, retaining walls), or **terraces only under streets and pads** (original cliffs kept, plan heights not honoured everywhere)? Does the town have to stay inside the gorge's walls? (section 4)
3. Kit filler: are low yard walls, arches and stalls acceptable "in Hlaalu style" where no building fits, or should unfilled lots stay empty ground?
4. Street surface: one paving class for all ranks, or distinct paving for main streets vs lanes?

## 8. Not verified

- Cost and success rate of `requests` on `settle` outputs (never run); whether `settle`'s `layout.json` includes `partition.programme`.
- Whether `ParcelConfig.params` overrides a preset per key (option C).
- Hlaalu footprint sizes, door counts, mesh bases (need S1).
- Whether MU street paths share exact node coordinates (needed for junction heights).
- Exact fields of Scrib's terrain stamp format v1 and the site raster spacing for arm B.
- CityEngine `i()` insert semantics, Houdini/Cities: Skylines grading details, talus 1:2 civil practice, Alt et al. citation, Parish/Mueller/Weber details (from memory or prior note, not re-read).
- cityforge facts are second-hand (prior note); no public source found by search.
- Game-unit conversion of cityforge caps (70 units per metre assumed).
