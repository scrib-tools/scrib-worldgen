# Magna Urbis for terrain, rivers and nature: what it offers and how we use it (2026-11-24)

Source: MU checkout `21e2217`. Paths are relative to `rust/crates/` unless they start with `docs/`, `web/` or `rust/bridge/`. V = read in source (file:line). U = inferred, not run. Nothing was built or run.
Our side: `worldgen.md` stages 1-8, `sf_worldgen`, `mw_worldgen`. Slope handling of towns is in `magna-urbis-slopes-2026-11-24.md` (MU's town planner never edits terrain).

## Recommended use for Morrowind 10x (adopt in this order)

1. **Water chain: adopt as-is.** Call `terrain::vector_water::condition_and_water` on our macro heightfield. It gives river centrelines with meanders, braids, oxbows, deltas and estuaries, lakes with a water budget, and the carved bed and valley. We have nothing like it (our `stream.rs` has no meanders or lakes; worldgen.md stage 5 only plans them). This is the biggest win.
2. **Keep anchors by masking MU's height change.** MU has no "keep this area" input anywhere (Q2). The chain returns `delta`, the whole height change. We apply `h' = h + delta * (1 - anchor_weight)`, then re-run our stage-4 stamps. We must also filter or reroute river lines that cross anchor footprints (we write that part).
3. **Roads between anchors: adopt as-is.** `core::settlements::Terrain::roads(seed, sites, cfg)` takes our fixed settlement centres and populations and routes roads on the terrain (Q4). It also gives nodes (inns, ferries, tolls).
4. **Forest canopy field as the tree density prior.** `terrain::build_forest_field` gives canopy cover, height, needleleaf share and a river-bank gallery band from climate and terrain. It is a display raster, not tree points, so our stage 8 stays the placer (Q3).
5. **Keep our own: macro uplift erosion (stage 2), mid-detail erosion (stage 3), runtime filter (stage 7), landforms, stamps, rock/flora scatter.** MU has no equivalent that takes our uplift/erodibility fields or anchors as boundary conditions.
6. **Do not adopt:** neural amplification (weights licence not granted, GPU/Python stack), Underdark (no caves in the showcase), Azgaar import (town recipes only), MU's procedural terrain-from-nothing (fixed 1024 grid, no external control).

Build facts: the `terrain` crate is pure Rust, no wasm-only code. Deps: `serde`, `serde_json`, `rayon`, `magna-urbis-random` (V `terrain/Cargo.toml`). Licence is AGPL-3.0-only (workspace, V `rust/Cargo.toml`). Toolchain pin 1.98.0, edition 2024 (V `rust/rust-toolchain.toml`). The `core` crate (settlements, roads, resources) adds `geom`, `delaunator`, `geo`, `i_overlay` (V `core/Cargo.toml`). Scrib (GPL) must use these only through files, never link them. Note "only" vs our "or-later": a combined binary is AGPL-3.0 only.

## 1. Inventory of MU terrain features

Common facts. Heights in metres, sea at 0, square raster, row-major from the north-west corner, cell centres, `width` in metres (V `terrain/src/lib.rs:1-2`, `engine.rs:417-420`). Only square maps. Everything is deterministic and seeded by a string (V; rows run in parallel but each writes its own slice, `forest.rs:9-11`).

| Feature | What it does | Inputs / outputs | Entry point | Rust or TS | Cost |
|---|---|---|---|---|---|
| Procedural terrain from scratch | 11 relief families (flat, hills, valley, canyon, mountains, mixed, plateau, high mountains, volcano, caldera, cavern) from noise, uplift, regional erosion, ridged detail, surface erosion | seed, width 500-100,000 m, relief, erosion dose, motif 250-50,000 m, 17 erosion overrides. Output is a fixed 1024² field whatever the width (V `docs/methods/compact-terrain-phasor.md`: "retained physical field is always 1024²") | `TerrainGenerator::new*` `engine.rs:57-175`; `TerrainPipeline` `erosion/pipeline.rs:31-220` (stages 0-7 via `advance`); `generate_terrain` `lib.rs:515` | Rust (also GPU path in TS) | no native figure stated |
| Uplift input | None external. Uplift is built inside the pipeline from noise and parameters (V `erosion/pipeline.rs:245-281`). `set_noise` only injects noise planes (V `:208`) | | | | |
| Erosion kernels (public) | `erode_grid`: implicit stream-power incision after a priority flood, depressions rise by `deposit` (V `priority_flood.rs:159`). `flood_grids`: priority-flood, D8 receivers, area, basins (V `:46`) | any height grid, `steps`, no mask | `priority_flood::{erode_grid, flood_grids}` | Rust | the conditioner runs it at 512² and upsamples the delta above 512 (V `vector_water/conditioning.rs:758-780`) |
| Hydrology engine (older "Rust legacy" chain) | priority-flood drainage, flats, depression breaching, lakes, rivers, meanders, estuaries, wetness | `HydrologyEngine::new(seed, width, n, &height, sea)` with n only 64-1024 (V `hydrology.rs:546`, `validate_field :1112-1124`); `generate(&HydrologyConfig)` `:652`. Output `HydrologyOutput` `:166-232`: receivers, accumulation, basins, filled, lake raster and rings, `adjusted_height`, river points stride 4 (x, y, width, water z), river meta, nodes | `hydrology.rs:546`, `:652` | Rust | browser: "10-25 s for a 1024² grid" without GPU (V `docs/synthese_rivieres_heightmap.md:141`) |
| **V2 vector water chain** (the one V2 uses) | undulations (marsh hollows) → conditioning (erosion, breaching, kept lakes) → water (budget lakes, river network with meanders, braids, anabranches, oxbows, deltas, confluence angles, headwaters) → river beds and valleys cut into the relief | `height` n×n (n 2-8192, V `wire.rs:253-272`), `width`, `Conditioning` (`conditioning.rs:5-40`), `Settings` (`vector_water/mod.rs:71-109`), `Climate{temperature, precipitation}` (`mod.rs:131`), seed. Output `ConditioningResult{height, delta, ...}` (`conditioning.rs:159`) and `Water` (`mod.rs:161-176`): lake mask, lake level, lake kind, river `Network` | `condition_and_water` `mod.rs:423`; `compute_water` `:178`; `condition_relief` `conditioning.rs:725`; framed `request` with ops `condition/chain/water/hydrology/rivers/field/delta/downsample` `wire.rs:253-470` | Rust (native and WASM) | same 10-25 s class at 1024²; no native timing stated (U for 2400²) |
| Rivers as vectors | `Line{points: [x,y,...] f64 metres, area, level, kind, id, joins, outlet}`; `Network{rivers, lake_links, oxbows, braids, distributaries, length_km, meander_km, deltas, avulsions}` (V `rivers.rs:46-100`). Width `3*(A/1e6)^0.45*scale` (V `rivers.rs` `channel_width`) | | `Water.rivers` | Rust | |
| Meanders | `Settings.sinuosity`, `oxbows` share kept as lakes (V `mod.rs:100-108`); migration slowed by bank height (terrain influence) | | `vector_water/rivers/meander.rs`; also `hydrology_meanders.rs` for the legacy chain | Rust | |
| Incision | `bed_depth(w) = clamp(0.45+0.05w, 0.6, 2.2)` m; `valley_depth(A) = min(2.5*(A/1e6)^0.4, 40)` m (1 km²: 2.5 m, 100 km²: 16 m); extra cut at most 4 m (V `incision.rs:14, 39, 20`); `incise_rivers` `:252` | | | Rust | |
| Lakes | kept depressions (area, depth, coverage caps), water-budget closed lakes (Turc-Pike), lake level plane, shore field | `Conditioning.lakes` 0-4; `lake_depth`, `lake_area`, `lake_coverage`, `max_lake_area` (V `conditioning.rs:5-40`); `lake_field` `lake.rs` | | Rust | |
| Coasts | 8-bit direction mask, island modes, coast profile, sea capsules fitted to the relief | `set_coast`, `configure_coast`, `set_coast_surface` `engine.rs:766-894`; `cut_coast_field` `erosion/pipeline.rs:6` applies a coast to any field | | Rust | not needed: our coast is given |
| Canyons | fixed tortuous drainage network and fractured cross-sections as a relief family | `canyon.rs`, `channels.rs` are `pub(crate)` (V `lib.rs:3-19`): reachable only through `relief = "canyon"` | | Rust | |
| Terrain zones | painted operation masks, 5 channels (sea, plain, hills, mountains, high mountains), 8-128² u8 (V `terrain_zones.rs:1-18`). "Never elevations." | `TerrainGenerator::with_zones` `engine.rs:512`; zoned surface `:453` | | Rust | |
| Neural amplification | Terrain Diffusion in Rust/Burn (coarse → base → decoder), wgpu/WebGPU | `neural` crate, `Request`, `Output` (`neural/src/pipeline.rs:154-262`); the amplification modes (Classic, Phasor, Hybrid, MSE) use a Python service (V `docs/methods/neural-earth-amplification.md:3`) | | Rust + Python | needs GPU |
| Azgaar import | one FMG burg to a town recipe (culture, biome, climate, river, coast side) | no terrain output (V `docs/AZGAAR_IMPORT.md`) | | TS only | |
| Underdark | turns any relief into caves: drainage becomes tunnels | `engine.rs:640-656` | | Rust | skip |
| Regional settlements and roads | place settlements on terrain, then route roads | see Q4 | `core/src/settlements/mod.rs` | Rust | roads route on a grid pooled to at most 512² by default (V `roads/mod.rs:73`, "1024² routes about ten times slower than 512²") |

Climate is two numbers (sea-level temperature °C, precipitation mm) plus presets; there is no biome map in the terrain crate (U: grep found none; biomes appear only in the Azgaar mapping and forest climate).

## 2. Terrain editing and the external-heightfield question

**Can MU take our heightfield? Yes.** Three entry points, all Rust:

- `TerrainGenerator::with_surface_owned(seed, width, Vec<f32>)` keeps an external signed surface "as is" with no procedural relief added; n any 2-8192 square (V `engine.rs:421-447`, `checked_source_n :9`). Output is read back with `sample_region` (`:928`) or `physical_height_field` (`:627`).
- `set_edited_surface(heights)` replaces regional elevations, keeps procedural detail, and runs `erode_surface` once over the whole field (V `engine.rs:569-590`). Refused on a prepared surface (`if self.prepared ... Err`, `:570`).
- `vector_water::condition_and_water` (the chain) takes any height grid directly (V `mod.rs:423`; the V2 web app sends imported heightmaps through it, V `docs/V2_TERRAIN_PARITY.md:19`).

**Can it keep fixed regions unchanged? No, not as an input.** I grepped `hydrology*.rs`, `vector_water/*`, `erosion.rs`, `terrain_zones.rs`, `engine.rs` for protect, pinned, frozen, mask, anchor, keep-out, locked: no public mask, weight or constraint argument exists. The only "protected" hits are internal (retained lakes in depression handling, `hydrology_depressions.rs:56`; frozen flat cells, `hydrology_flats.rs:378`). `TerrainZones` weights change what the generator does by region, not what stays fixed. Sea cells are kept: the conditioner copies every height <= 0 unchanged (V `conditioning.rs:795-797`).

**Editing in the app is a delta layer, TS only.** `terrain.modification` holds metre offsets applied once after generation and coast and before hydrology; the source is kept apart so edits never bake in (V `docs/design/RELIEF_PAINT.md`). Brushes: raise, lower, level, smooth, restore, with radius, softness, strength; level and smooth keep a fixed fine residual (V `web/src/v2/reliefPaint.ts:1-60`; `paintHeight :47`, `paintZone :71`). "Draw relief" is a separate coarse drawing at most 128² that goes through refinement and erosion; a sketch has `processing: 'surface' | 'erosion'` (V `rust/bridge/terrainSketch.ts:1-6`). These are about 6 KB of TS; nothing in them is needed for a batch tool.

**How we keep anchors (the key question).** Because the chain returns `delta` (V `mod.rs:420-423`: "the whole change of the relief (hollows, conditioning, beds and valleys)"), we can mask after the fact:

1. Build the macro field with our stage 2 and stamp the anchors (stage 4), so anchors are already in the input and rivers see them as ground.
2. Subtract our sea level so the sea is 0 (MU's datum, V `engine.rs:417`).
3. Run `condition_and_water`.
4. Output `h' = h + delta * (1 - w)` with `w` our anchor weight (1 on the footprint, smootherstep to 0 across the band). Result is bit-exact inside footprints (U: arithmetic is ours).
5. Read `Water.rivers` lines and drop or reroute any course through a footprint; a river that ends at a footprint needs our own bridge or ford rule. MU does not know about anchors (U).
6. The river `level` values per point (water surface) must also be corrected where we cancelled the bed cut (U).

Open risk (U): MU's valley cut can be up to 40 m deep; masking the delta at an anchor edge can leave a step. Our band blend already handles the same issue for stamps.

## 3. Nature assets

**Trees: a canopy control field, not trees.**
- `build_forest_field(&ForestParams)` (V `forest.rs:334`) takes seed, climate `[T, P]`, extent, `n`, `height`, optional routing water codes (0 land, 1 sea, 2 lake, 3 river) and river segments (nine floats per segment), `enabled`, `density` 0-2, `clearings` 0-1, `progressive` (V `forest.rs:296-313`).
- Output `ForestPlanes` (`:315`): RGBA8 at about 16 m per texel, max 1024² (V `forest_field_resolution :292`). R canopy cover 0-1, G potential canopy height 0-50 m, B needleleaf fraction, A stand tone (V `docs/methods/render3d-vegetation.md:51-60`). A second `bank` raster is the riparian gallery band, up to 48 m wide (V `forest_bank.rs:11`; `beach` is a crown-root grid for the shader, `forest_beach.rs:1`).
- Eligibility terms: rock, slope, perennial snow, treeline, climate; clearings from noise (V doc `:160-275`). "Display only: never feeds terrain" (V `forest.rs:9-11`).
- No species, succession, understorey or crown meshes. The doc lists these as limits: "no species composition... crowns are material only" (V doc `:865-900`). Crowns are drawn per fragment in the shader (Godot/WebGPU), so there are no tree points anywhere in the Rust output (U: grep for placement points in `forest*.rs` found none).
- Tree meshes and impostors are a Godot-side experiment (V `docs/surface/VEGETATION_ASSET_NEXT.md`, `TREE_ASSET_EXPERIMENT.md`): Poly Haven CC0 assets, not in the Rust crate.

**Rocks: appearance only.** `docs/surface/ROCK_CLOSE_DETAIL.md` is a slope-projected PBR shading change; `TERRAIN_MESOSCALE.md` adds bounded height residuals (±3 m) from rock/fragment context. Both are Godot shader plus Python preparation. No rock placement exists.

**Geology and soil fields (not assets, but useful for scatter).** `core/src/resources/`: realised geology (8 rock classes from a regional prior, `geology.rs:1-12`), soil fields on a 60/120/240 m block grid from one priority flood: HAND, TWI, alluvial and colluvial potentials, soil depth, stoniness, moisture (V `soil.rs:1-6`), and deposit anchors (`anchors.rs`). Rust. Meant for MU's rural land use and activity sites, but they are generic fields.

**Rivers, lakes, coasts as nature.** Covered in Q1. Riparian band and lake interiors come from `forest_bank.rs`.

## 4. Mapping to our pipeline and what to do

| Our stage | Our file | MU piece | Decision |
|---|---|---|---|
| 1 Placement | `anchors.rs`, `x10.rs`, `anchor.rs` | none | ours |
| 2 Macro form (uplift + stream power, anchors as boundary conditions) | `landform.rs`, `stream.rs`, `mfd.rs`, `flood.rs`, `landscape.rs` | `TerrainPipeline` stages 1-2 (regional evolution) | **nothing usable**: no external uplift, erodibility or anchor input (V `pipeline.rs:245-281`). Keep ours. |
| 3 Mid detail (pipe model, thermal, landform operators) | `thermal.rs`, `cliffs.rs`, `stratovolcano.rs`, `landforms/` | MU surface erosion is 1024² stream-power plus thermal | ours |
| 4 Anchor stamps | `stamps.rs`, `valley_terrace.rs` | none (no grading anywhere, slopes note) | ours |
| 5 Hydrology: rivers, lakes | none yet (`stream.rs` only routes) | `vector_water::condition_and_water` + `incise_rivers` | **use as-is** (call the crate from scrib-worldgen), mask delta (Q2). Pin rev 21e2217. |
| 5 Meander centreline (Paris 2023 plan) | not built | `rivers/meander.rs`, `Settings.sinuosity`, `oxbows` | **use as-is**, drops the planned port |
| 6 Material fields | `geology.rs`, `layers.rs` | `resources/soil.rs` (HAND, TWI, soil depth, stoniness) | **port ideas/adapt** on demand: our slope/curvature/flow fields already cover splat inputs; HAND/TWI/soil depth are new |
| 7 Runtime detail | `filter.rs`, `tiles.rs/.wgsl` | none (MU's fractal detail is TS in `terrainAdjust.ts`) | ours |
| 8 Nature scatter | `scatter.rs`, `flora.rs` | `build_forest_field` (cover, height, needleleaf, riparian band) | **adapt**: use as an optional density prior and river-bank band; our learned occupancy tables still place Morrowind's own species and rocks |
| Settlements | `mw_worldgen` anchors | `Terrain::place` `core/src/settlements/mod.rs:488` (positions can be fixed: `SettlementSpec.position`, `:30-40`) | **use as-is** to turn anchors into sites with population and culture |
| Roads between settlements | none | `Terrain::roads(seed, &[RoadSite], &RoadConfig)` `:506`; `RoadSite{key, population, culture, center, radius}` `roads/mod.rs:86`; `RoadConfig` `:30` (level, surface, density, grade weight, crossing cost, sharing, exits, `grid` default 512) | **use as-is** (needs `Raster{height, water, water_level, river_width, river_ids}` `mod.rs:139`, which the water chain produces) |
| Coast | given | `cut_coast_field` | nothing, we keep Vvardenfell's coast |
| Neural amplification | not adopted | `neural` | nothing |

Dependency notes:
- Neural weights: the nine bundled student weights have **no established licence grant** (V `THIRD_PARTY_NOTICES.md:162`); the teacher is MIT. Not usable for shipping. The Python modes need an external service.
- `HydrologyEngine` is capped at n 1024 (V `hydrology.rs:1115`). Use the vector chain (n up to 8192) instead.
- Grid size at 10x: 60 km at 25 m is 2400² (worldgen.md "Scale"), inside the chain's n range. The conditioner erodes at 512² and upsamples the delta, so it scales; river tracing and meanders run at full n (U for time and memory; measure once, one run).
- The roads router pools the raster to 512² by default; at 60 km that is about 117 m per cell, which is coarse for roads between close towns (U; set `grid` higher and accept the time).
- Climate is two scalars; Vvardenfell's regions (Ashlands, Bitter Coast, West Gash) need per-region temperature and precipitation. We would run the chain once with an average climate or per region and merge (U).

## 5. Gaps where neither side has anything

1. **Anchor-aware water.** MU rivers and lakes do not know about anchors; we have no hydrology at all. Needs our rule: routes avoid footprints, or cross with a bridge/ford record.
2. **Named rivers and fixed coast as inputs.** MU has no "this river must exist along this line" input (`Settings.main` is auto/none/stream/river/major, V `hydrology.rs:60-64`; U for the vector chain). Our guide has them as strokes; neither side consumes them in a water stage.
3. **Rivers below grid resolution.** At 25 m cells a 5 m brook is sub-cell. The vector line is exact, but the carved bed (0.6-2.2 m) is a grid effect. Stage 7 or `sf_water` must carve channels from the polylines at 2 m.
4. **Tree and rock instance placement.** MU gives canopy rasters only; we have learned scatter (stage 8) but no climate/treeline eligibility for a world that has no Morrowind references (new regions at 10x).
5. **Non-square maps, multi-tile water.** MU is single-square. A 60 km square is fine; anything bigger or tiled needs our own stitching, and global drainage cannot be tiled (MU's own doc says independent windows cannot supply upstream discharge, V `docs/methods/neural-earth-amplification.md:16`).
6. **Biome map.** No biome layer in the terrain crate; vegetation is climate scalars + terrain terms.
7. **Settlement site conditioning.** No cut, fill, walls or ramps in MU (slopes note). Ours stays: `valley_terrace` for build-time pads and stair/ramp geometry.
8. **Sea level and datum.** MU assumes sea at 0 and positive land floors (min land 0.3 m in the procedural path, V `docs/methods/procedural-generation-stages.md` row 12). Our guide must be shifted; U for the chain on negative land.

## First actions (cheap, no new systems)

1. Add `magna-urbis-terrain` (git, rev 21e2217) to scrib-worldgen; write a thin `water` tool: guide folder in, `delta`, lake mask, lake level, river polylines, river meta out as files.
2. One run on the existing 60 km, 2400² Vvardenfell guide with default `Settings` and an average climate; log time, memory and river count; apply the masked delta; check that no footprint changed by a bit (log lines only, no pictures; the owner checks the look).
3. Feed the anchors' centres and populations to `Terrain::roads` and compare against the Morrowind road network the owner knows.
