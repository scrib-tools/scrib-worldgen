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

## Open questions (owner, player-facing; not needed for Milestone 1)

- Interiors of generated buildings: none, locked doors, or generated interiors.
- NPCs and services in new quarters.
