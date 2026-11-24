# Magna Urbis capabilities: the map of what we do not use yet (research C-1, 2026-11-24)

Scope: Magna Urbis (MU) at rev `21e2217`, read-only. Question: which MU features exist, where their output lives, what we use,
and the order to adopt the rest. Companion notes (not repeated): `magna-urbis-{roads,terrain,slopes}-2026-11-24.md`,
`settlements-m2-placement-2026-11-24.md`.

Path prefixes: `U/` = `rust/crates/core/src/urban/`, `S/` = `rust/crates/core/src/settlements/`, `L/` = `rust/crates/core/src/landuse/`,
`H/` = `rust/crates/houses/`, `W/` = `web/src/` (MU's TypeScript, not in the Rust crates). Every `file:line` is in the checkout
`~/.cargo/git/checkouts/magna-urbis-b198125f6ef17db3/21e2217/`. "Ours" = `D:\Gamedev\scrib-worldgen\crates\`.

## 0. Findings that matter before the table

1. **We call 2 of MU's ~12 stages and read 4 fields of its richest output.** `settle/run.rs` calls `generate_quarters` and `generate_parcels`
   (`U/mod.rs:110`, `:136`) and `building_sites::requests` (`U/building_sites.rs:1671`). `lots.rs:120-215` keeps from a request only `accesses`,
   `programme.floors`, `preview`, `preview_parts`, plus the allocation's role, function, wealth. Everything else a request carries is dropped:
   `programme.massing` (volumes, roofs, cover, wall, tint, footing), the culture and pack, the terrain patch, `party_boundaries`, `occupation`,
   `feasibility` (`building_sites.rs:1628-1637`).
2. **`layout.json` already contains MU's own fortification, and nothing reads it.** The stored medina run
   (`C:\cargo-target\scrib\worldgen\k3-fit-lot\medina\layout.json`) holds 2 `layout.enclosures`: style `rammed-earth`, material `rammed-earth`,
   curtain 4 m, height 8 m, 2 gates, 37 `towers_detail`, 26 `reserve` shapes. Medina gets this from `fortifications.json` group `islamic`
   (`fortifications.json:38-40`) with no option set by us (`fortification` defaults to `"culture"`, `U/config.rs:60`). `filler.rs` towers, walls and
   arches are a second, worse wall on top of it.
3. **Every `LayoutConfig` / `ParcelConfig` option is reachable from the recipe with zero code.** `run.rs` deserialises `[magna_urbis.layout]` and
   `[magna_urbis.parcels]` straight into MU's structs (`settle/run.rs:107-110`); unknown keys are refused (`deny_unknown_fields`,
   `U/config.rs:8`). We set 6 keys today (tissue, maxStairGrade, maxLaneGrade, density, backBuildings, styleSheet houses+plots).
4. **All MU data files are compiled in** (`include_str!`: `U/morph.rs:~100` `catalogue()`, `U/enclosure.rs:15`, `U/fortification/style.rs:23`,
   `U/forms/kit.rs:195`). A new culture is a vendored-file edit, not a runtime file. This matches the owner rule (vendored, adapted through data).
5. **The density clamp is confirmed but is not the only door.** `let f = (1.0 / density).clamp(0.5, 2.0).log2()` (`U/morph.rs:137`) caps the
   density multiplier at 2 (and `resolve` accepts 0.1..10, `:118-120`, then silently clamps). `parcels.params` may set `houseArea`, `frontage`,
   `plotDepth`, `coverage`, `wideLotChance`, `lotMergeChance` per zone directly (`catalogue.json:2995-3036` `fields.parcels`; applied at `U/morph.rs:~165-190`),
   which bypasses the clamp for lot size. `buildDepth`, `roomDepth`, `setback`, `sideGap`, `infill`, `arch`, `buildingOp` are catalogue-only (section 3).
6. **Lot-wall geometry is not in the Rust crates.** `partition.enclosure` carries only resolved numbers (`U/enclosure.rs:21-39`, set at
   `U/pipeline/parcels.rs:153`: for medina plot 0.95, front/side/back 1, wallHeight 2.2). The yard walls, fences and hedges are built by the
   web renderer, `W/render/cityLotEnclosures.ts` (1155 lines) and `W/render/hedges.ts` (105). The "MU garden walls" cited in `filler.rs:5` do not exist
   as Rust geometry. Using this feature means porting that TS into the vendored MU, generic and upstreamable.
7. **Names and Azgaar import are TypeScript only.** Toponyms: `W/gen/names/` (549,469 names, 51 generators, 8 MB JSON,
   `docs/NAMEBENCH_TOPONYMS_FR.md`); Azgaar: `docs/AZGAAR_IMPORT.md` (web page input). The Rust crates have only district role labels (French,
   `U/services.json:458`, `UrbanDistrict.label` `U/types.rs:207`).
8. **The harbour planner refuses cliff shores by design**: gentle bank only, "about 80 m within 150 m" (`U/macroplan/harbour.rs:234`), no apron down a
   cliff (`:347`, test `:1979-2006`), "not at a cliff foot" (`:781`), cliff shore refused (test `:2012`). Confirmed.
9. **Fantasy and Morrowind-relevant cultures already exist** among the 115 catalogue cultures (`catalogue.json:129778`): elven, dwarven, wizard-city,
   gnomish, stilt-town, orcish, halfling, drow-enclave, duergar-hold, myconid-colony, giant-hold, goblin-warren, lizardfolk-swamp, merfolk-tidal,
   necropolis, nomad-camp, norse-ringfort, kraal, plus historical ones and modern/sci-fi (cyberpunk, solarpunk, post-apocalyptic-ruins...). House forms
   `tree | mushroom | longhouse | yurt | battered | mage | stilt` are a trait (`forms/styles.json:3` `_doc`), so a Telvanni mushroom tower and an
   Ashlander yurt are data.

## 1. Capability table

Status: **used** (we read the output or set the option), **partial**, **ignored** (MU produces it, we drop it), **rebuilt** (our file duplicates it),
**absent** (not in MU's Rust). Output paths are fields of `Layout` (`U/types.rs:140`), `Partition` (`:384`), `UrbanStructure` (`:167`) or a request
JSON (`building_sites.rs:1628`).

| # | Capability | What it does | Key options | Output (file:line) | Our status | Value for a game world |
|---|---|---|---|---|---|---|
| 1 | Fortification: curtain, towers, gatehouses, works | Traces a wall per culture style on the town's land, fits terrain, places gates where streets cross, towers by spacing, ditch, outworks, lean-tos | `fortification` (culture/none/wall/palisade), `fortificationStyle`, `wallStiffness`, `wallTerrain`, `wallRelief`, `wallTowerSpacing`, `moat` (none/dry/wet), `ditchWetness`, `plannedWall`, `heightsCastle`, `wallExtension` (concentric/attached/auto), `portEnclosure`, `riverControl`, `dryDitches`, `medievalWalls`, `oldEnclosureCitadel` (`U/config.rs:60-110`); data `fortifications.json` | `layout.enclosures[]`: `path`, `gates`, `inner_gates`, `style` (palisade, gord, hedge, curtain, flanked, concentric, rammed-earth, wengcheng, bastioned), `material`, `curtain_width_m`, `height_m`, `towers_detail[]` (shape round/square/d/rect, kind corner/flank/gate/water, footprint), `gatehouses[]` (timber/tower/twin-tower/bab/ravelin/water/postern, shape, passage, height), `bastions`, `outworks`, `ditch`, `attached`, `reserve` (`U/types.rs:28-126`) | **ignored** in layout.json; **rebuilt** by `filler.rs` towers/walls/arches | Real city walls with gates on actual streets, river works, castles on knolls; Morrowind: Imperial forts, Redoran/Dunmer fortress towns, Vivec cantons; fantasy: dwarven, orcish, wizard walls |
| 2 | Enclosures of lots and fields | Per culture yard walls, fences, hedges around plots and field boundaries | `plotEnclosure`, `fieldEnclosure`, `countryside_enclosures`, `enclosureMaterial` (`U/config.rs:509-527`); data `enclosures.json` (default `:4`, groups `:5`, cultures `:25`) | `partition.enclosure` = numbers only (`U/enclosure.rs:21`, set `U/pipeline/parcels.rs:153`). Geometry: TS only (`W/render/cityLotEnclosures.ts`) | **partial** (numbers present, unread); street-wall walls **rebuilt** by `filler.rs` | Closed courtyard street fronts (medina `walledCourt` 0.95), bocage vs open field, hedges; gap: needs a Rust port of the TS |
| 3 | Building massing | Raises each preview piece into volumes with floors, eave, roof kind and ridges, cover, wall, tone, tint, parapets, features, hill footing | `houseComplexity` simple/complex (plans: bar, ell, tee, court, cross, aitch), `lowRuralHouses`, `outbuildings`, `backBuildings`, `singularDensity`, kit data `forms/styles.json` kits | per request `programme.massing[]` and `volumes[]` (`U/massing.rs:1-40` contract; attached at `building_sites.rs:1611-1625`; `footing` on sloped pieces) | **ignored** (request is built, field dropped at `lots.rs:120`) | Silhouette, roof, wall and storeys per building: pick the right prefab or generate; tint for colour; storey caps by size and distance from core |
| 4 | `houses` architectural engine | Generates a full building (bodies, rooms, walls, openings, roof pans, access, materials) from 267 sourced recipes in 32 packs, inside a lot | `HouseConfig` seed/pack/recipe/mode (strict, inspired, fantasy)/era/size/variation/wealth (`H/src/model.rs:48`); data `H/data/recipes.json`, `urban-admission.json` | `houses::placement::place_batch(PlacementBatchRequest)` (`H/src/placement.rs:652`) consumes the `requests` we already build; wired in `rust/crates/local/src/main.rs:1098`; result `PlacedBuilding`, `enrich_placement` (`:2143`) | **absent** (we do not call it); owner: later, from here first | Procedural meshes in the culture's idiom; Morrowind: AF-MAG (medina) recipes B01-B11 now, own pack later; fantasy: `workshop` free presets (20 plans, 22 roofs) |
| 5 | Programme: roles and services | Counts building roles per culture family by population (`per`, `minPop`, `max`, `site` nucleus/gate/waterfront/hill/edge/spread, `area`), reserves whole blocks for landmarks, allocates plots | `programme.{intent,template,functions,resources,period,mode,navigableAccess,lore,state,decay,counts,countMode}` (`U/programme/model.rs:12-47`); data `services.json` (`cultureServices :5`, `cultures :480`, `families :572`, `nucleus :463`, `harbour :366`, `rural :362`, `isolated :352`) | `partition.programme` and `layout.programme`: `demands`, `reservations`, `allocations[]` (role, label, function, households, residents, wealth, landmark), `adjacency`, `environment`, `counts` (`U/programme/model.rs:104-153`) | **partial**: allocations read only for role/function/wealth (`lots.rs:215`); no `programme` options set | Mosque, hammam, kasbah, funduq, tannery, granary placed by rule (medina run: 22 distinct roles in 921 allocations); counts forceable (`counts`, `count_mode=force`); lore landmarks |
| 6 | Weighted functions and intent | Six biases (agricultural, commerce, artisan, riverine, port, capital) and isolated intents (farm, wayside temple, shrine, mill, inn, tower) | `programme.functions`, `programme.intent` (`U/programme_functions.rs`, `docs/CITY_VARIATION_VARIABLES.md` 1.1) | same allocations; isolated intents give one building | **ignored** | A port, market or capital town differs by data; Balmora as `commerce`+`port`+`capital` |
| 7 | Wealth, districts, centres | Districts get a functional role (civic, market, religious, craft:<trade>, port, elite, commoner, poor, garrison, gardens, foreign), wealth 0..1, centres with weights | `suburbs`, `phases`, `shantyScale`, `gateVillage(s)`, `U/roles.rs` | `layout.structure.districts[]`, `centers[]`, `port_districts[]`, `history[]`, `metrics`, `capacity` (`U/types.rs:167-326`) | **ignored**; `layout.structure` is absent from the stored medina layout.json (verify: serialised only for some mechanisms) | Quarters with identity (tanners, smiths), rich/poor split drives house size and storeys; Balmora's districts |
| 8 | Floors, storeys, housing capacity | Floors from wealth, pressure, zone, distance; household floor need; capacity plan densifies then extends the footprint then borrows a donor tissue | `density`, `zoneDensity`, `smallPlaceCapacity`; `U/footprint/capacity.rs` | `programme.floors`, `allocations[].housingFloorNeed`, `structure.metrics.capacity` (`U/types.rs:263`) | **partial** (floors read `lots.rs:129`) | Population actually housed; a verifiable "12,000 people fit" |
| 9 | Harbours and waterfront | Continuous quay, piers, moles, harbour types (merchant quay, fishing, shipyard, staithe, canal port, mole harbour, naval arsenal, timber port, ferry landing...), boats by culture family, cranes, cargo, posts, slips | `portType` (auto/bare/id), `portDetachment`, `shelteredHarbour`, `portShelter`, `harbourBridgeClearance`, `programme.navigableAccess`; data `harbours.json` (`types :4`, `families :80`, `boats :209`) | `structure.quays[]` {kind, shape, deck_z_m, material} (`U/types.rs:184`, `U/macroplan/harbour.rs:1460`), quay streets role `quay`, `structure.waterways` | **ignored** (planner runs, nothing exported); cliff shores refused | Docks, quays, boats, cranes for Seyda Neen, Vivec, Balmora's river front; a quay-less cliff shore needs a vendored generic patch |
| 10 | Canals and lagoons | Turns street share into canals/rii, quays both sides, locks, feeder, basins | `lagoonCanals`; `U/lagoon.rs` | `structure.waterways[]` {shape, level_m, kind canal/rio/pool/mouth/feeder/basin, crossings} (`U/lagoon.rs:38`) | **ignored** | Vivec-style canal cantons; Balmora's Odai canals |
| 11 | Countryside: farms, fields, orchards | Context-driven farm plans (type, yard, outbuildings), field furlongs and strips, hedges, tracks, orchards, gardens, meadows, marsh, commons, farm drives routed by cost | `L/mod.rs:83` `Input` (biome, cost, soil, hab planes), `farmDensity` (placement config), `denser_smallholdings`, `outbuildings`; data `catalogue.json` `farmstead` (6 keys), `horticulture-ts.json` | `landuse::generate_rural(&Input) -> RuralOutput.layer`: `areas[]` (kinds field, meadow, pasture, forest, orchard, garden, marsh, commons), `farmsteads`, `reserve`, `ways`, `headlands`, `natural_ground` (`L/mod.rs:132`, `:208-229`) | **ignored** | Farmland, orchards, hedged lanes round every town; Ashlander/Dunmer farm yards; saltrice paddies = field kind with culture data |
| 12 | Rural and remote sites | Dispersed farms in parent farmland, wayside shrines, chapels, monasteries; remote places (summit temple, cliff monastery, sanctuary at a rock foot, crag tower, hermitage on a sea stack, tomb in a gorge, lost ruins) with access none/track/cliff-stair/rock-steps/tunnel/causeway/rope-bridge | `Config.farm_density`, `Config.remote_places` (`S/mod.rs:56-135`); `activities.json` (`sites` 85 templates, `remote`) | `Output.activities[]: ActivitySite` {template, mode, parent, x,y, ground, access, access_class, buildings[], installations[], situation, approach} (`S/activities.rs:27`, `S/countryside.rs`, `S/remote.rs`) | **ignored** (roads crate says "no farms or activity sites yet", `roads/src/run.rs:145`) | Morrowind's hermit shacks, tombs, shrines, Ashlander camps, remote Daedric/Dwemer sites as generated places |
| 13 | Activity sites and ground installations | Mills, mines, quarries, fishing huts, sheepfolds, logging camps on resource anchors; weirs, leats, quarry faces, spoil heaps, racks, piers, corrals | `resources/` anchors (`rust/crates/core/src/resources/anchors.rs`), `activities.json` (340 archetypes, 20 looks, 42 family looks, 11 ground kinds) | `ActivitySite.buildings`, `.installations` (`S/installations.rs`, `S/nodes.rs` for road nodes: toll houses, ferries, inns every 15-25 km, pilgrim stops, border posts) | **ignored** | Egg mines, ebony mines, mills, ferries and inns on the Silt Strider roads as data-driven sites |
| 14 | Regional settlement placement and population | Scores land (flat, water, coast, defence, flood, resources), places a rank-size (Zipf) set of places with culture, class and site archetype (harbor, bridge, hilltop, spur, pass, island, lakeshore, valley, plain) | `Config` (`S/mod.rs:56`: `mode` auto/counts/list, `distribution` zipf/classes, `main`, `list`, weights, `clustering`, `site_weights`); `catalogue.json` `sitePrefs` (`:130692`); `docs/REGIONAL_POPULATION_FR.md` | `Terrain::place -> Output.settlements[]` {spec, center, cls, archetype, radius, scores} (`S/mod.rs:181-200`) | **ignored** (our sites come from `sites.json` / guide; `settlements-m2` doc studies placement only) | Founding new places on generated land (10x Vvardenfell), populations from a rank-size law; `list` mode keeps Morrowind's real towns fixed (`position`) |
| 15 | Settlement kinds and size grammar | 11 native kinds (single, ruin, farmstead, camp, hamlet, street-village, nucleated-village, market-town, walled-town, city, metropolis) chosen by demographic class | `settlementForm`, `siteArchetype(s)`, `fixedSites` (`U/config.rs:35-48`), `S/demographic_classes.json` | `layout.archetype`, `layout.structure.{mechanism,family}` | **partial** (auto) | Hamlets to metropolis from one engine; ruins; camps |
| 16 | Roads and street network | Streets by rank with widths, grades, stairs, bridges; regional roads between places | see `magna-urbis-roads-2026-11-24.md`; `maxStreetGrade`, `maxLaneGrade`, `maxStairGrade`, `allowStairs`, `parallelMerge`, `townMainRoad`, `roadNetworkPrune`, `streetImprovisation`, `villageStreets`, `deepBlockLanes` | `layout.streets[]` {path, widths, rank, role}; `S/roads/` | **used** (roads crate, regional.rs), most polish flags unset | Roads, stairs, bridges |
| 17 | Terrain and water chain | Relief, erosion, coast, hydrology, lakes, rivers, canyons, forests, underdark carving | see `magna-urbis-terrain-2026-11-24.md`; `rust/crates/terrain/src/` | heights, water planes | **used** (water crate) | Valley reshape; canyon and forest modules unused |
| 18 | Cultures, presets, style sheets | 115 cultures, 623 presets (culture/zone recipes); hybrids by style sheet per facet streets, plots, houses, monuments, rural (weighted cultures, per zone or quarter) | `styleSheet`, `mixCulture/mixMode/mixShare`, `phaseOverrides`, `tissue`, `zone`, `recipe`, `preset` (`U/config.rs:490`, `U/house_mix.rs:1-8`) | resolved per lot: `Allocation.culture`, `architecture_pack` | **partial**: houses+plots facets only; works (today's finding) | Morrowind houses as a hybrid today, a real culture entry later (section 3) |
| 19 | Building mass roles and kits | Roof/pan/wall/storey rules per mass label, kits per architectural family | `forms/roles_{core,modern,monuments_a,monuments_b,rural}.json`, `forms/styles.json` kits (`:7`), `derive` (`:72`), `cultures` (`:83`) | feeds #3 | **ignored** | A Telvanni tower or Hlaalu stack as one data block (`styles.json:3`) |
| 20 | Monuments and singular buildings | Landmarks per role (cathedral, mosque, keep, forum, wizard tower, mausoleum) with forms by culture | `monument`, `singularDensity`; `U/parcels/monuments/`, `services.json nucleus :463` | allocations with `landmark=true`, massing | **partial** (allocated, not read) | Temples, tribunal sites, castles by rule |
| 21 | Funerary, springs, shanty, fringe, gate villages | Cemeteries and mausolea (`funerary.rs`), spring baths/wellheads (`springs.rs`, `spring_water.json`), informal outskirts (`shanty.rs`), frontage along the ring road (`fringe.rs`), markets at gates | `shantyScale`, `gateVillage` | `structure.spring_sites[]`, `structure.shanty`, `structure.gate_village_centers` | **ignored** | Outskirts, baths, graveyards with no hand placement |
| 22 | Decay, ruin, period, lore | Settlement `state` (thriving/declining/abandoned/ruined), `decay`, empty buildings shuttered/roofless/stubs; `ruinShare` by culture and class; `period` filter; `lore` tokens (temple, palace, necropolis, magic) add landmarks | `programme.state/decay/period/lore`; `services.json states :343`, `ruinShare :350`; `programme_tokens.json` | allocations with `.abandoned` role suffix, ruined massing (`U/parcels/ruins.rs`, `massing.rs:329 ruin_piece`) | **ignored** | Abandoned Dwemer/Daedric sites, ruined Imperial forts, declining towns |
| 23 | Per-lot, block and quarter edits | Stable overrides keyed by parcel id / block index / quarter index: culture, role, function, households, wealth, tissue, density, zone, new shape, split into 2..64 parts, delete, reroll | `LayoutConfig.overrides` (`U/config.rs:268`), `ParcelConfig.overrides` (`:538`); `U/overrides.rs:18-191`; `docs/URBAN_EDITING_FR.md` | applied in place; `partition.prepared` blocks, `layout.config.block_edits/quarter_edits` | **ignored** | Hand-keep a landmark lot or district; "mod layer" over generated towns; stable ids across reruns |
| 24 | Underdark and caves | Carves any relief into tunnels, caverns, galleries; cave towns pack land per home; cave fortification; biome `underdark` in land use | `options.layer`, `S/CAVE_*`, `U/fortification/cave.rs`; `docs/UNDERDARK.md` | terrain: `rust/crates/terrain/src/underdark.rs`; `CapacityPlan.cave_pack` (`U/types.rs:263`) | **absent** (no tunnels, "no Underdark" `roads/src/run.rs:145`) | Dwemer ruins, ash caves, kwama mines, Daedric undercroft |
| 25 | Camps and nomads | Ring, concentric and cluster camps with tents/yurts; `natural_camp_approaches` | `campGrowth`, `campRingWobble`, `U/kinds_camp_*`, `parcels/camp*.rs` | layout + allocations (huts typology round/long) | **ignored** | Ashlander camps |
| 26 | Names | Cultural toponym grammars (51) over a 549k name pack | `W/gen/names/*` | none in Rust | **absent** (TS) | Place and district names; a port of the data/grammar is needed |
| 27 | Azgaar FMG import | Builds a town from a burg | `docs/AZGAAR_IMPORT.md` | web only | **absent** | Not needed for Morrowind; generic fantasy worlds from FMG maps |
| 28 | Ground materials and vegetation data | Ground composition rules from relief/water/resources; canopy and landscape cover areas | `docs/GROUND_MATERIAL_MAPPING.md` (design study), `L/ground.rs`, `L/mod.rs:380 generate_natural_cover`, `render_geometry/city_ground.rs` | `layer.natural_ground`, `landscape_ground` regions; render-side clip | **ignored**; ground materials are a design, not code | Land-cover polygons under towns and farms to feed Scrib's ground and groundcover rules |

## 2. Replace our add-ons

### 2.1 `fit.rs` (kit best-fit into footprints)

- **What MU gives instead.** The footprint is MU's (`preview`, `preview_parts`, `occupation.rs` `Kind` Row/Detached/Court/Insula/Pavilions/Hanok/...
  `U/parcels/occupation.rs:22-37`; on a lot whose sides are within 15 degrees of square the footprint is the lot band itself, keeping party walls,
  `:1-12`). On top of it, `programme.massing[].volumes` (ring, floors, `eave_m`, `rise_m`, roof, ridges, cover, wall, tone, tint) is the silhouette
  (`massing.rs:1-40`), `footing` handles slopes (way plinth/terrace/stilts; `building_sites.rs:1611-1625`), `terrain.{ground_z_m,gradient,water_z_m,
  max_foundation_step_m}` is in the request (`:1637`), `occupation {coverage, depth_m, frontage_m}` states the tissue's own size (`:1388-1394`).
  Full geometry: `houses::placement::place_batch` over the same requests (`H/src/placement.rs:652`).
- **How to read it.** Keep `requests` (already called, `lots.rs:159`), serialise per building: `programme.massing`, `culture`, `programme.composition`,
  `party_boundaries`. A kit fit then selects a prefab by volume dims, storeys, wall/cover and wealth instead of re-testing hulls in a footprint.
- **Gap.** MU has no prefab or mesh-kit concept: it makes geometry (houses) or volumes (massing). So `fit.rs` does not disappear in the prefab path;
  it shrinks to a generic "volume to catalogued model" table (data), and it becomes unnecessary once `place_batch` meshes are exported. The measured
  mismatch (Hlaalu 16 x 10 m houses fit 84 of 507 medina footprints) is fixed at the source by data: `houseArea`, `frontage`, `plotDepth` (params) and
  `buildDepth`, `roomDepth` (catalogue) sized to the kit (section 3), not by widening the fit domain to `lot`.

### 2.2 `merge.rs` (join narrow empty lots until a kit house fits)

- **What MU gives instead.** Lots are born the right width: `frontage`, `houseArea`, `plotDepth`, `wideLotChance`, `lotMergeChance` per zone
  (`catalogue.json:5615` medina/core: frontage core [4.6, 6], houseArea core [70, 240], wideLotChance 0.2, lotMergeChance 0.05; all overridable through
  `parcels.params`, `fields.parcels` `catalogue.json:2995`). MU's own merging: `parcels/consolidation.rs` ("merging slivers, joining lots and absorbing
  leftover land"), `parcels/court_pairs.rs` (two lots share a court); `densify`/`block_cut.rs:196-241` re-cut deep blocks by the `houseArea`.
  A continuous street wall is `buildingOp: shopRow` + `party_walls` and `programme.frontage_continuity` (`building_sites.rs:409-420`: row 1.0, courtyard 0.85).
- **How to read it.** Set the params; read `parcel.shape`, `request.party_boundaries`; no post-pass.
- **Gap.** MU has no "merge to a target width" operator, only probabilities and ranges; target-width merging is not a MU feature. The honest replacement is
  a generic vendored parameter (target lot width from the culture's `frontage`), not our merge. Remove `merge.rs` once lots are cut to the right
  `frontage` range.

### 2.3 `filler.rs` (street-wall pieces, arches, towers)

- **What MU gives instead.** City walls: `layout.enclosures[]` (curtain `path`, `towers_detail`, `gatehouses` with `passage` and `passage_width_m`,
  `bastions`, `outworks`, `ditch`, `reserve`; `U/types.rs:28-126`). Gates (the arches) are `Gatehouse.kind` timber/tower/twin-tower/bab/ravelin/water/
  postern, placed on real street crossings (`fortification/gate_budget.rs`, `crossings.rs`, `dead_gates.rs`; a gate no street uses is closed,
  `U/pipeline/complete.rs:34`). Corner and gate towers: `towers_detail` kind `gate|corner|flank|water`, shapes round/square/d/rect, spacing
  `towerSpacing` (`fortifications.json:10` style curtain `[25,35]`; override `wallTowerSpacing`). Lean-tos: `attached[]`. Wall of lots: `partition.enclosure`
  numbers plus the TS geometry (`cityLotEnclosures.ts`).
- **How to read it.** `layout.enclosures` is already in `layout.json`. A kit adapter maps `style/material/height_m` to wall pieces and
  `towers_detail`/`gatehouses` to tower and gate prefabs, laid along `path` by the piece length (this laying is the only part a kit needs; the where
  and what is MU's).
- **Gap.** (1) Lot street-front walls between houses: MU's Rust has numbers only; port `cityLotEnclosures.ts` into the vendored MU. (2) Our "arch across a
  lane mouth" has no MU equivalent (MU's arches are wall gates); lane gates are not a MU concept. (3) MU walls enclose the town circuit, not the medina's
  per-block street wall; the medina street wall is the lots' own walls (`plot 0.95, front 1`).

## 3. Morrowind culture as data

A Morrowind culture is a set of entries across files, all compiled in. Line numbers are those of the pinned rev.

| File | What to declare | Fields | Lines |
|---|---|---|---|
| `U/catalogue.json` `cultures.<id>` | label, recipes by zone, plan, nucleus, enclosure per zone, phase counts, faubourg share, village/hamlet street rules, render, growth, district naming, site preferences, shanty | `label`, `districts{language,unit,anchor,per,names}`, `recipes[{id,label,zone,preset}]`, `plan{pattern,enclosure,fortification,gates,orientation,stairs,waterfront}`, `nucleus{kind,shape,area,compound,ring}`, `core/ring.enclosure{shape,wall,fossil,towers}`, `phaseCount [[pop,n]...]`, `faubShare`, `village.street{green,depth,spread,backLane,fieldLanes}`, `hamlet.form`, `render.towerShape`, `sitePrefs{harbor,bridge,...}`, `growth`, `shanty` | section starts `:129778`; medina `:129994` (recipe presets `:130009`); `sitePrefs` examples `:130692`, `:131377` |
| `U/catalogue.json` `presets.<culture>/<recipe>.params` (one per zone recipe: core, ring, faubourg, village, hamlet) | the tissue and **house sizes** | `houseArea{zone:[min,max]}` m2, `frontage{zone}` m, `plotDepth{zone}` m, `buildDepth{zone}` m (footprint depth, `U/parcels/occupation.rs:2469`), `roomDepth [lo,hi]` m (wing thickness, `:2473`), `setback`, `sideGap`, `coverage`, `infill`, `blockSize`, `density{zone}` (households/ha), `arch{typology,roof,storeys,material}`, `buildingOp`, `plotOp`, street keys (`streetOp, closeOp, orientation, widthByRank, deadEndRatio...`) | presets start `:3037`; `medina/core` `:5615` |
| `U/catalogue.json` `fields` | which keys `params` may override | quarters: 26 keys (streetOp, closeOp, orientation, laneSpacing, wardArea, accessDepth, fieldTwist, curvature, fieldNoise, fieldWavelength, fieldRandom, gridSpacing, gridSkew, blockSize, minBlock, minWidth, widthByRank, widthScale, widthJitter, placeThreshold, deadEndRatio, slitDepth, plotDepth, contourFollow, contourBlend, longCut); parcels: 10 keys (plotOp, streetOp, houseArea, coverage, frontage, plotDepth, plotTilt, wideLotChance, lotMergeChance, deepFill); validated at `U/morph.rs:~165-195` (other key: error "n'est pas utilise par l'etape") | `:2995` |
| `U/catalogue.json` `farmstead`, `shanty`, `lotSolidity`, `districts` | farm buildings (width, depth, eave, stilts, lean), informal quarters (covers, walls), district naming pools | per culture key `farmstead`; `shanty` 4 types | keys at top of file (`catalogue.json:1-3036` before `fields`) |
| `U/forms/styles.json` | house vocabulary and exterior traits, no code | `kits.<id>.house` (plan weights, growth, extras, roofs, overhang, stack), `derive[]` (rules by roof/cover), `cultures.<id>.{kit,traits}`, `traits{tradition{roof,pitch,storeys,cover,wall,gableToStreet,form},mediterranean,flavor,bodyScale,metricScale,form,formShare,massiveStorey,piles,openness,era,roleGroups}`; Telvanni: `form: mushroom`/`tree`/`mage`; Ashlander: `form: yurt`; bodyScale/metricScale for non-human folk | `_doc :3`, `kits :7`, `derive :72`, `cultures :83` (medina `:115`) |
| `U/forms/roles_*.json` | mass-role rules (wall height, base, roof, tiers, material per mass label), per-culture overrides | role entries, `overrides.<culture>` | `roles_core.json`, `roles_monuments_a/b.json`, `roles_rural.json` |
| `U/services.json` | building roles per family and culture: which monuments, counts, sites | `cultureServices.<culture>` (`:5`), `cultures.<culture>` -> family (`:480`, medina `:483` = islamic), `families.<family>` role lists (`:572`), `nucleus.<kind>` (`:463`), `states` (`:343`), `ruinShare` (`:350`), `rural` (`:362`), `harbour` (`:366`), `labels`/`districtLabels`, `occupancy` per family (householdSize...) | as listed |
| `U/enclosures.json` | lot and field enclosures | `default :4`, `groups.<g>.{plot,field,material,fieldMaterial,front,side,back,yardMerge,wallHeight,inset,cultures[]}` (`:5`, `walledCourt` lists medina `:7`), `cultures.<id>` (`:25`), `village` sub-object, `lotLines` | as listed |
| `U/fortifications.json` | wall style per culture | `styles.<style>.{kind,material,curtainWidthM,heightM,towerHeightM,towerShape,towerRadius,towerSpacing,cornerTurn,gate,minorGate,ditch,ditchP,ditchWidthM,outworks,attached}` (`:6-16`), `riverWorks` (`:17`), `wallStiffness` (`:24`), `groups.<g>.{heightsCastle,wallStiffness,innerEnclosure,wallExtension,cultures[],styles[{style,weight,minPop,maxPop}]}` (`:25`, islamic `:38`), `cultures.<id>.{styles,towers,curtainWidthM,heightM,towerSpacing}` (`:67`) | as listed |
| `U/harbours.json` | harbour types and family looks | `types.<id>.{sites,minPopulation,berth,fill,boats,cranes,cargo,posts,anchored,moles,jetties}` (`:4`), `families.<f>.{cultures[],material,crane,boats,types{id:weight}}` (`:80`, islamic `:99-100`), `boats` (`:209`) | as listed |
| `U/activities.json` | farm/activity/remote templates and culture families | `sites` (85), `archetypes` (340), `looks`, `family_looks`, `ground`, `remote`, `cultures.families` (islamic `:544`) | as listed |
| `U/native_witnesses.json`, `U/functional-source-coverage.json`, `H/data/urban-admission.json`, `H/data/recipes.json` | admission witnesses, functional-source coverage, house recipes per pack (`width_m`, `depth_m`, `floors`, `bodies`, materials, roof) | recipes `AF-MAG-B01..B11` (`H/data/recipes.json`): width 5-9 m x depth 7-12 m, floors 1-2 | files |

**What only code accepts (so a vendored, generic patch is needed, written to go upstream):**

- Culture -> architecture pack and land family: `U/culture.rs:54` `MAPPINGS` (medina at `:64`: `"courtyard", "AF-MAG", false, "town", "courtyard"`).
  A culture with no entry is not admitted by the programme stage. Fix: read the mapping from `catalogue.json`.
- `buildingOp` kinds (`U/parcels/occupation.rs:22-37, 66-110`: Row, Detached, Farm, Court, Insula, Pavilions, Hanok, Yashiki, Round, Long, Hall, Warehouse) and
  the recognised `plotOp` list (`U/morph.rs` after override: burgage, courtyard, siheyuan, machiya, compound, garden, wholeBlock) and `closeOp` list
  (closes, culDeSacTree, hutong, roji, none) and `streetOp` (organic, grid).
- Params not overridable at run time: `buildDepth`, `roomDepth`, `setback`, `sideGap`, `infill`, `arch`, `buildingOp`, `cornerFill`, `courtyardMin`,
  `bigCourtChance`, `footprintConformity` (only in the preset; `fields` lists exclude them, `catalogue.json:2995`; read at `U/parcels/occupation.rs:2437-2474, 3471-3566`).
- Density multiplier cap `[0.5, 2]` (`U/morph.rs:137`): raise by data (a per-culture `densityMax`) or set `houseArea`/`frontage` directly.
- Roof, wall, cover vocabularies (massing.rs:20-30): `roof` gable, hip, shed, pyramid, cone, tent, dome, barrel, ring, flat; `wall` timber...ashlar; adding
  a new material (basalt, ash-brick, bone) is a code enum, not data.
- Tint palettes: `forms/styles_modern.json` (`kit.rs:147`); historical cultures carry no `tint`.
- Plot-enclosure geometry (TS only). Names (TS only).

## 4. Order of adoption (one agent slice each)

1. **C-2 Read MU's fortification and drop `filler.rs` towers/walls/arches.** Export `layout.enclosures` (path, gates, `towers_detail`, `gatehouses`,
   `ditch`, `reserve`) into a `fortification.json` beside `placements.json`; set `fortification`, `fortificationStyle`, `moat`, `wallExtension` in the recipe
   (zero code, `[magna_urbis.layout]`); map `style/material/height_m` to kit pieces in an adapter. Check the `reserve` shapes against lots (MU reserves
   the wall land). Verify with `stats` counts, no screenshots.
2. **C-3 Export the full building request** (`programme.massing`, volumes, cover/wall/tint, `footing`, `party_boundaries`, `occupation`, culture/pack, allocation
   label/households/landmark, `programme.deficit`) into `lots.json`; `fit.rs` picks by volumes. No MU change. Gate: lots.json diff only.
3. **C-4 Programme and district options and export.** Set `programme.{functions,state,decay,counts,lore}` for Balmora (commerce+port+capital); export
   `structure.districts`, `centers`, `quays`, `spring_sites`, `waterways`, `shanty`; find why `layout.structure` is absent in the stored medina layout.
4. **C-5 Morrowind culture entries (vendored, data).** Fork edit: `catalogue.json` cultures+presets with house-size params from the Hlaalu/Redoran/
   Telvanni kits, `forms/styles.json` kit+traits (mushroom, yurt), `services.json`, `enclosures.json`, `fortifications.json`, `harbours.json`,
   `activities.json`; plus the code patch: `MAPPINGS` read from `catalogue.json`, and a per-culture `densityMax`. Start by extending today's working style
   sheet to the `streets`, `monuments`, `rural` facets while C-5 is written.
5. **C-6 Replace `merge.rs`**: size lots by `frontage`/`houseArea` (params), `shopRow` + party walls; measure footprints fitting vs the 84/507 baseline; delete
   `merge.rs`.
6. **C-7 Lot enclosures**: port `cityLotEnclosures.ts` + `hedges.ts` geometry (yard loops, gates, fence styles) into vendored MU Rust (`partition.enclosures`),
   generic, tested; then drop the street-wall wall pieces of `filler.rs` (the file goes).
7. **C-8 Harbour**: `portType`, `portDetachment`, export `structure.quays`; vendored generic patch for cliff shores (stepped/landing types exist:
   `harbours.json` "stepped landing", `beach-landing`; extend `U/macroplan/harbour.rs:234` gentle-bank rule by data).
8. **C-9 Countryside**: call `landuse::generate_rural` per town with `Input` from our grid (cost, water, soil planes), export `areas`, `farmsteads`, `ways`;
   use `outbuildings`, culture `farmstead` data.
9. **C-10 Regional placement and activities**: `Terrain::place` in `list` mode (real Morrowind towns fixed by `position`, generated hamlets, farms, remote
   places, nodes); feed `Output.road_sites()` to the roads crate (activity access routed).
10. **C-11 Decay, ruin, lore**: ruins and abandoned sites for Dwemer/Daedric/Imperial places (`state`, `ruinShare`, `lore`), `isolated` intents.
11. **C-12 Houses**: `place_batch` over the requests, export meshes/plan data; kit fit becomes the fallback (a style-to-prefab table).
12. **C-13 Edits layer**: per-lot/quarter overrides (`ParcelOverrides`, `QuarterOverride`) as the mod layer for hand-kept lots; stable ids.
13. **C-14 Underdark** (terrain carve, cave towns) and **names** (port the TS grammar/data pack, or adopt the pack JSON) as separate slices; Azgaar import
    only if a generic fantasy front end is wanted.

Order logic: 1-4 only read what MU already computes (cheap, remove our duplicates, no MU change except config); 5-8 are vendored data and generic code patches;
9-13 widen what is generated; 14 is optional breadth.

## 5. What contradicts the earlier findings

- "Density is clamped at 2" is true but incomplete: lot size is not bound by it (`parcels.params` houseArea/frontage/plotDepth/coverage bypass the clamp;
  `U/morph.rs:137`, `catalogue.json:2995`).
- "Our add-ons rebuild MU's fortification and enclosures badly" is true for fortification (MU's wall exists in our layout.json, section 0.2). For lot
  enclosures MU has no Rust geometry: the original is TypeScript, so there is nothing to read until it is ported.
- `layout.structure` (districts, quays) is absent from the stored medina layout.json, so a part of MU's output may not be produced in the mode we run;
  slice C-4 must check this first.
