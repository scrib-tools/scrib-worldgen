# Magna Urbis: regional roads and settlement placement, and how we use them (2026-11-24)

Pinned rev 21e2217. Paths are under `rust/crates/core/src/` unless said otherwise. V = read in source (file:line). U = inferred.

## Recommended use (roads for Morrowind 10x)

1. **Use MU's road router as is, with our fixed settlements.** `Terrain::roads(seed, &[RoadSite], &RoadConfig)` (`settlements/mod.rs:506`) takes the sites you give it. It never moves them: "road options never move a settlement" (`settlements/mod.rs:3`). We skip MU's site picker for anchors and feed the kept Morrowind towns (and Balmora 2.0) as `RoadSite { key, population, culture, center, radius }` (`roads/mod.rs:86`). V.
2. **Do not route the whole 10x island on one MU raster.** The raster is at most 1024 cells a side (`mod.rs:416`) and roads route on a grid pooled to 512 by default (`RoadConfig.grid`, `roads/mod.rs:57`, `mod.rs:511`). At ~90 km that is 90-175 m per routing cell: too coarse for switchbacks and bridges. V for limits; the 90 km size is U. Plan: (a) route the whole network once on the coarse grid to get topology, hierarchy and classes; (b) re-route each link at ~10-20 m on a corridor crop with two fixed end sites. Part (b) is our code around MU's `route`. U.
3. **Turn off what we own:** `exits` = 0 (an island has no map-border gateways; defaults are main 1, local 1, `roads/mod.rs:70`), `network_nodes` off or kept as a gameplay hook, `tunnels` 0, `activity_access` off until we have farms. V (fields exist).
4. **Convert MU's output to town input ourselves.** MU's own converter is TypeScript (`rust/bridge/urbanRoads.ts:6`, `regionalStreets`, ~50 lines). It splits each segment at its crossings into `Street { path, widths, rank, role }` with role `regional`, `bridge`, `ford` or `causeway`. Port it to Rust in `scrib-worldgen`. Then pass the result to `urban::generate_quarters(..., roads)` (`urban/mod.rs:107`) for new towns. V.
5. **For kept towns, the road ends at the town's street exits.** MU routes to the site centre and the town stage clips it. For a kept town we must end the road at the authored exit, not the centre. Our side: clip the road at the anchor radius and snap it to the nearest authored exit (see Q3). U.
6. **Roads in 3D are all on our side:** heights along the path, grading, bridge decks, meshes and trails (Q5). MU gives only 2D polylines.

## Q1. What the region stage does

**Entry points.** Placement: `Terrain::place` (`settlements/mod.rs:488`). Roads: `Terrain::roads` (`mod.rs:506`), which calls `roads::route` (`roads/mod.rs:736`), then `roads::enforce` (safety net: no road on open water, no dangling end) and `nodes::place`. V.

**Inputs.** `Raster` (`mod.rs:139`): `width` (metres, 250 to 200000), `n` (16 to 1024), `height` f32, `water` u8 (0 dry, 1 sea, 2 lake or cave rock, 3 river), `water_level`, `river_width` (m), `river_ids` (-1 off rivers). Square only. Validation `mod.rs:414-435`. V.

**Site choice.** `placement::place` (`placement.rs:784`) scores every cell: flatness, buildable land, fresh water, river, coast, flood, defence, elevation, culture and site-type weights, access from regional entries, and room for the town plan (`score`, `placement.rs:201`; `site_room`, `:320-345`). Rank-size (Zipf) populations from a `main` population, or per-class counts, or an explicit `list`. Archetypes such as harbour or bridge sites come from `archetype_index` (`:32`). Radius from population: `extent_radius` (`mod.rs:655`). V.
**Fixed sites in placement:** mode `"list"` with `SettlementSpec.position = Some(p)` keeps the position as given and only warns if unsuitable (`placement.rs:854-861, 988-999`). V.

**Which towns get linked.** `plan()` (`roads/mod.rs:202`): Delaunay triangulation of the sites, then its minimum spanning tree, then a Delaunay edge is added as a shortcut when the tree path is longer than `ratio` x straight distance. Ratio comes from `density`: 0 gives tree only, 0.5 gives 1.6, 1 gives 1.15 (`:~820`). Places under 150 people use a milder ratio. Links are routed most important first (importance = smaller population of the pair). V.

**Routing algorithm.** `search.rs`: least-cost search (A*-style with a 4-ary heap) on the grid. V.
- 24 move directions (8 neighbours, knight moves, (1,3) moves) so a road can climb at a third of the slope (`search.rs:3-4`). State = cell plus heading sector (8). Turn limit per class 45 degrees, turn cost on the square of the angle and dearer on steep ground, so hairpins land where the slope eases (`:9-14`, `:72-78`).
- Cost per move: distance x slope factor x ground factor x meander + turn cost. Slope factor is `1 + gw*(5g + 80g^2 + ...)` with extra penalty beyond the class grade limit (`search.rs:~673-676`; `gmax = class_grade`, `roads/profile.rs:185`). Grade limits come from the culture profile (medieval 0.10; roman 0.08; local x1.2, track x1.5, trail x `trail_grade`).
- Ground factor: shore and flood-plain multiplier (`land`, `roads/mod.rs:~790-800`, uses distance to water and habitability). Rivers cost 30-40 x `crossing_cost` x culture `bridge` factor; lakes 20 x. **Land use, forest and biome are not costs** (no use in `roads/`; only farm reach). V by grep.
- Reuse: moves along existing roads cost `discount` = `1 - 0.8*sharing`; cells beside a road cost more, so routes merge into trunks or keep apart (`search.rs:5-7`).
- After the search: smoothing to the grade limit (`smooth_path`, `roads/mod.rs:1220`), bridge decks straightened to meet rivers at 45 degrees or more (`deck.rs:12-16`), planarisation into one graph, dead-end pruning, traffic assignment over all Delaunay pairs (`network::assign_traffic`). V.

**Hierarchy, classes, widths.** Classes `highway, main, local, track, trail` (`roads/profile.rs:4`), widths 9 / 6.5 / 4.5 / 3 / 1.5 m times development level and culture (`:7`). Class from the trips a link carries (population of smaller end: 5000+ highway, 1000+ main, 150+ local, 15+ track, below that trail, shifted by level `LEVELS` `:207`). Surfaces dirt, gravel, paved, flagstone from level, class and culture (`:253`). Levels: primitive, rural, developed, imperial. V.

**Bridges and fords.** A new crossing is allowed only over rivers narrower than the class limit `BRIDGE_SPAN` = 400 / 300 / 200 / 100 / 40 m (`search.rs:84`). Wider water is crossed on a bridge a stronger road already built, or not at all. Sea and lakes are closed, except causeways for cultures with `causeway` (shallow lakes). Crossing kind per wet run: `causeway` (lake), `ford` (river under 3.5 m wide and class track or trail), else `bridge` (`deck.rs:55-71`). Output is a `Crossing { kind, path }` per segment. A river crossed more than once gets dearer (`few_crossings`, `search.rs:737`). V.

**Switchbacks.** No explicit switchback object. They emerge from the 24-move search, turn limits and the grade cost. Measured grade is reported per segment. V.

**Output** (`roads/mod.rs:126-176`): `RoadOutput { segments, junctions, warnings, unreachable, exits, nodes, approaches }`. `RoadSegment { class, surface, profile, width, a, b (junction ids), path: Vec<Point>, crossings, length, grade, importance, tunnels }`. `Point` is `{x, y}` in metres, 2D only; **no heights in the output**. Ground height is `Fields::height_at` internally (`grid.rs:525`). V.

## Q2. Fixed settlements and our own terrain

- **Yes for roads.** Call `Terrain::new(Raster)` with our height and water, build `RoadSite`s from our anchors and new towns, call `roads()`. The call examples exist: `core/examples/activities_probe.rs:100,145`. V.
- **Only for roads: skip `place()`.** `roads()` does not need a `place` result; `Output::road_sites()` (`mod.rs:204`) is just a helper that maps settlements to `RoadSite`. V.
- **Inputs to build from our site format:**
  - Raster: our site folder has `heights.f32` and `water.u8` (1 = open water) only (`crates/settle/src/site.rs:12-50`). MU needs river codes 3, `river_width` and `river_ids`. We must derive them from our hydrology (we have the river network at terrain time; the settle crop does not carry it). Without river widths MU cannot price or classify bridges. U.
  - Frame: MU raster row 0 is the first row in `y` increasing; ours is row 0 = north with y north. One flip, same as `grid.rs` already does for the town stage. V for ours; the row convention of `Land` is U (check `index`/`pos`, `grid.rs:545-551`).
  - Sites: key, population (sets class and Delaunay importance), culture (road profile: need a Morrowind row, see below), centre, radius.
- **Culture catalogue:** profiles are a fixed table `PROFILES` of 9 (`roads/profile.rs:29`). Morrowind cultures need a row or a reuse of an existing profile. Same patch we already plan for towns. V.
- **Seam if we want more control:** none needed for sites. Smallest seams for roads: (1) per-link fixed endpoints are already supported (`connect(from, to, start, ...)`, `roads/mod.rs:402`); (2) there is no public "route this one pair" API, so for the corridor re-route we call `roads()` with two sites on a cropped `Terrain`. U.

## Q3. How a road meets a town

- **Region side:** the region stage does **not** use town exits or gates. A road ends at the site centre cell; cells inside the site radius are free of cost (`settled`, `roads/mod.rs:820-834`, `search.rs:637`). Remote places only have an `approach` line (`RoadSite.approach`, `roads/mod.rs:104`). V.
- **Town side:** `generate_quarters(seed, sites, config, surface, roads: &[Street])` (`urban/mod.rs:107`) takes the regional roads. It first runs `network::bridge_water` on them (`:113`), clips to the town envelope, and uses them as founding axes (`axis_alignment = "regional"`, `footprint/site.rs:138`). Roles `regional | approach | bridge | ford | causeway | outbound` are treated as network by gate budgeting (`fortification/gate_budget.rs:42`, `dead_gates.rs:42`). Gates are chosen where regional roads pass the wall. `ensure_regional_access` (`urban/network/exits.rs:392`) routes up to 4 links if the built town has no contact with a regional road (searches within 1500 m, lane grade limit). Regional spans (bridge, causeway, approach) within 1.6 x radius are kept in the layout (`pipeline/finish.rs:107`). V.
- **Neighbours:** `Site.neighbours: Vec<Claim>` (kept land and edge streets) is how a town joins an existing town (`urban/types.rs:420`, `neighbours.rs`). Our `claim.rs` uses it. V.
- **Order is road first, town second.** The town grows to meet the road, not the reverse. For a kept anchor the reverse is needed: the town's exits are fixed and the road must meet them. MU has no input for "this exit point at this bearing". U.
- **Did our Balmora runs duplicate MU?** `crossing.rs` does not duplicate the region stage: it makes a regional road when we have no region stage (a site folder carries no road). It mimics what the region stage hands to a town, and relies on MU's `bridge_water` for the deck (its own header says so, `crossing.rs:1-10`). Once real MU regional roads exist, `crossing.rs` becomes a fallback only. V for the header; the rest U.

## Q4. Unfinished or broken, per the two docs

Both docs (20 Nov 2026) are about the **town street graph**, not regional routing. Nothing in them reports a defect in the region router itself. V (read headings and summaries).
- ROAD_GENERATION_AUDIT: 148 of 196 junctions with more than 4 arms touch a bridge, approach or causeway; seven approach records meet one deck at one spot; bridge and gate alignment is poor (423 of 711 single-deck ends deviate over 45 degrees); 14 of 26 saved layouts carry the warning "impracticable regional run rerouted or omitted"; repairs after the fact are slow (bridge selection 394-850 ms). Rank 1 proposal: a selected abutment owns its deck axis and landing (audit section 5).
- STREET_GRAPH_DESIGN: junctions have no identity (street = polyline), repairs re-node the whole town; 13 deliveries planned; step 5 gives each selected bridge its approaches. Not implemented; the doc is a roadmap. Some counts were later corrected: land gates over 45 degrees fell from 43/154 to 4/84.
- **What this means for us (U):** bridge landings at town edges are MU's weak spot. Regional bridges are straightened to 45 degrees (`deck.rs:12`) but not graded, and nothing here covers the regional road itself beyond that.
- Also noted in code (V): roads are 2D and no embankment or deck height exists; `BRIDGE_SPAN` caps new crossings at 400 m for highways.

## Q5. What MU lacks for game-ready roads

| Need | MU today | We add | Where |
|---|---|---|---|
| Road height profile | none; 2D `Point` only. Internal `deck_height` over rivers (bank level) exists but is private (`grid.rs:582`) | Sample our heightfield along the path; for bridges, bank-to-bank height | worldgen tool (writes road files) |
| Terrain grading (cut/fill, embankments) | none. Town planner adapts to slopes, never edits terrain (our slopes note). Region router only avoids slope | Carve a corridor: flatten across the road width, blend to ground over a side distance, keep grade limit; write as a height delta raster | worldgen tool (bake into the terrain it hands to Scrib), so the game sees one heightfield |
| Bridge geometry | kind plus 2D `path` with one dry abutment point each side (`deck.rs:55-71`) | Deck height, pier or arch choice from span and class, ramp, kit models | tool emits bridge records; Scrib game side places the kit meshes |
| Ford, causeway | tagged only | Water-depth fit, stepping stones or raised bed | tool plus game |
| Road mesh or decal | none (web renders from polylines, `web/src/render/roadSurface.ts`) | Ribbon mesh or terrain-splat layer from class, width, surface | Scrib game side (renderer; we already have a roads texel layer idea in `roads.u8`) |
| Trails and paths | classes `track` and `trail` exist; `activity_access` links farms and mills as tracks (`roads/mod.rs:46`); dead ends are pruned at 150 m (`STUB_METRES`) | Footpaths for NPC routes, shortcuts, animal tracks: our own | tool |
| Junction detail, signposts, inns | `nodes` places tolls, ferries, inns, border posts (`roads/mod.rs:49`, `nodes.rs`) | Use as gameplay hooks; map to Morrowind content later | tool |
| Slope-aware routing against our own mesh | MU grid cost at 512 cells | Corridor re-route at fine resolution (Recommended use 2) | tool |
| Forest, bog, rock as route cost | none | Add a per-cell multiplier input; MU has only `land` from water and habitability. Needs a small patch to `Costs.land` (`search.rs:129`) | patch of vendored MU |
| Reproducible tunables | MU constants live in code (`BRIDGE_SPAN`, `TURN_METRES`, profile table) | Expose through our tunables file | tool |

## Q6. Did `crates/settle` reimplement MU? (quick check)

| Our file | MU equivalent | Verdict |
|---|---|---|
| `crossing.rs` (founding road across the narrowest water) | Region stage makes this road (`roads/mod.rs` crossings); town stage `bridge_water` bridges it | Substitute for a missing region stage. Not a copy: needed because a site crop has no regional road. U/V |
| `claim.rs` (kept town as `Claim`: footprint rings, edge streets) | `urban/neighbours.rs` `Claim` and its own raster closing (our header says it reuses the same method with our constants) | Adapter and re-implementation of the claim builder from raster input; MU builds claims from its own towns. Kept (V, header) |
| `site.rs` (reads the site folder) | `site.rs` in core (`core/src/site.rs`) is MU's own site hand-off; not compared in detail | Ours is the Scrib file contract. Do not merge. U (MU file unread) |
| `grid.rs` (crop to a `Surface`) | `urban/surface.rs:15` `Surface` is the MU type; ours fills it | Adapter, no duplicate. V |
| `run.rs` | Calls `generate_quarters`, `generate_parcels` directly | Uses MU, no duplicate. V |
| `filler.rs`, `merge.rs`, `fit.rs`, `lots.rs` | MU has narrow-lot pairing and garden walls in `urban/parcels` and `building_sites` | Kit fitting is ours by design (prefab kit). Partial overlap with parcels logic, named in headers. V (headers) |

## Open points to verify before building

1. Row orientation and unit of `Raster.height` (metres assumed from `route_height`): read `grid.rs:331-560` before the adapter.
2. Whether MU's coarse pooled routing (`Land::pooled`, `grid.rs:87`) keeps river IDs and widths properly for narrow rivers: its own comment says a creek narrower than the routing grid is "clipped" (`roads/mod.rs:~1070`).
3. Runtime on 1024 cells x ~100 sites: MU says 1024 is ~10x slower than 512 (`roads/mod.rs:57`). Measure once, do not repeat-bench.
4. Licence: our tool is AGPL, MU is AGPL-3.0-only: calling and porting are fine. The game side must only read the road files (no MU code in Scrib).
