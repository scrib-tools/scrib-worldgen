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

## Contract: files only

Scrib (GPL) writes a **site folder**; this tool (AGPL) reads it and writes a **layout folder**; later a placements
file becomes a generated mod that Scrib loads like any mod ("everything is a mod"). Neither side links the other.

Site folder, format 1 (metres, x east, y north, raster row 0 = north edge):
- `site.json`: `format: 1`, `name`, `producer`, `origin_m: [x, y]` (world position of the north-west corner of
  raster cell (0,0)), `cell_m` (raster spacing), `dims: [w, h]`, `water_level_m`, `centre_m: [x, y]` (where the
  town's centre should be), `source` (free text, e.g. "vanilla Balmora" or "x10s guide"), optional `claim`
  (below), `blake3` of each raster.
- `heights.f32` (little-endian f32, row-major, `w*h`, metres), `water.u8` (1 = open water), `roads.u8` (1 = ground
  textured as road or path, optional).
- `claim` (arm A only): `footprints: [{ id, kind, polygon: [[x, y], ...] }]` with `kind` one of `building`, `wall`,
  `bridge`, `stairs`, `other` (the exporter's classification; polygon = convex hull of the reference's mesh vertices
  projected on the ground, world-transformed), `doors: [{ id, at: [x, y], facing_deg }]`.
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
  else left out. Test: round-trip of the format, Balmora's footprint and door counts printed, water on both sides of
  its centre.
- **ST-2 (Scrib, GPL): site export from the 10x guide.** Same command, source `x10s`: a crop around the Odai reach
  nearest Balmora's mapped position, heights without the anchor stamp, upsampled to the site spacing; no claim.
- **ST-3 (this repo): `settle` CLI on Magna Urbis.** Reads a site folder + recipe, builds Magna Urbis's surface,
  claim and site, runs quarters and parcels, writes the layout folder and sheet. Test: a synthetic site fixture (slope,
  straight river, three footprints) gives lots, none inside the claim land, identical output for one seed.
- **ST-4 (runs): the Milestone 1 sheets** for arms A and B, a few cultures each, handed to the owner.
- **Later:** Morrowind cultures (vendored Magna Urbis); prefab kit catalogue from Morrowind's meshes (footprint, door
  side, storeys); placement on lots; the generated mod; the two scenes in game.

ST-1 and ST-3 run in parallel (the format above is the contract); ST-2 after ST-1.

## Open questions (owner, player-facing; not needed for Milestone 1)

- Interiors of generated buildings: none, locked doors, or generated interiors.
- NPCs and services in new quarters.
