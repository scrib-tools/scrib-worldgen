# Magna Urbis on steep ground: what it does, what it never does (2026-11-24)

Question (owner): "Magna Urbis has all the tools to handle terrain deterministically, no?"
Source: MU checkout `21e2217`, paths below relative to `rust/crates/core/src/urban/` unless prefixed `MU/`.
V = read in source (file:line). U = inferred or not run.

## Short answer

MU never changes the heightfield. It adapts the plan to the terrain it is given: street grades with a stair rung, contour-following streets, relaxed house slope gates, terrace wards, and a per-building footing record (plinth / terrace with a retaining wall / stilts). Our site failed for a reason we can change from the recipe, not because MU lacks the tools: the stair rung and the terrace tissue are off for our cultures. We did not set them, and (U) did not read the layout warnings that would have said so.

## 1. Does MU modify or grade terrain?

No, in any stage that touches a town.

- Outputs carry no heights. `Street` is `{path: Vec<Point>, widths, rank, role}` (V `types.rs:8-13`, 2D points). `Layout` holds quarters, streets, blocks, warnings, enclosures, phases, structure (V `types.rs:140-184`). No heightfield, no per-vertex z.
- The only grading-like output is per building, in the building request: `massing[i].footing = {way, base_z_m, low_z_m, drop_m}` from a drop of 1.0 m (V `massing.rs:393`, `405-420`). `way = terrace` adds `terrace: {ring, wall_m}`, a level pad run 1.5 m past the building downhill, plus the height of its retaining wall (V `massing.rs:396-404`, `TERRACE_APRON_M`). Ways: `plinth` (default), `terrace` and `stilts` by culture trait or pile culture; largest drop before the home is turned or made shallower: 2.5 m masonry, 1.5 m light walls, 4 m terrace and stilts (V `massing.rs:373-391`).
- Homes are turned on their lot to follow the contour, then made shallower; "a home is never removed for its slope" (V `hillside.rs:1-7`). Foundation step limit 0.75 m, widened to 6 m for a slope-relaxed town (V `feasibility.rs:22-29`; used `building_sites.rs:1487-1490`).
- Stairs are a street role string `"stairs"` on an ordinary 2D path, no step geometry (V `access.rs:226-227`, `quarters/subdivision.rs:683-686`). No ramps, no embankments, no cut and fill.
- `terrain/` is world generation only: canyon, channels, coast, erosion, hydrology, forest, underdark (V file list `MU/rust/crates/terrain/src/`); a grep for "urban" in it finds nothing (V). It never sees a town.
- The TS app has one terraced-terrain item, `web/scripts/terrace_feasibility.ts`: a CPU benchmark of cut and fill height deltas whose header says "does not change any retained terrain" (V line 1). It is a measurement, not a feature. `docs/surface/URBAN_GROUND_SUPPORT.md` only assigns ground materials under built cells (V, "changes only the derived bare-ground policy"). The word grading, retaining or embankment appears in no urban doc (V grep of `docs/*.md`).
- The terrain given to the town is not edited either: the pipeline swaps in a `stilt_ground` or raster-shoreline planning surface for lakes and stilt towns (V `pipeline/mod.rs:100-119`, `geography.rs:632`); the physical surface is kept unchanged.

## 2. How MU plans on steep ground

All of it is automatic and deterministic (no randomness beyond the seed), and `Surface` is the only terrain input (V `surface.rs:15-23`).

1. Grade is measured as the steepest local gradient along the path, not the mean (V `routing.rs:120-176`). A path that crosses a face obliquely has a lower grade than the face, so zigzags are legal.
2. Growth: a land cell is urban only if a street can reach it; "a slope is climbed by switchbacks along the contours or not at all" (V `footprint/growth.rs:462-467`). Service grade is the stair grade when `stairs_allowed`, else the street grade (V `footprint/generate.rs:215-220`). If the first growth takes under 25 % of the budget, an emergency ladder retries at `service.grade.max(max_stair_grade).max(g)` for g in 0.5, 0.75, 1.0, 1.5, keeping a retry that doubles the land (V `generate.rs:253-277`). Note it widens growth even for non-stair cultures.
3. `stairs_allowed` = `cfg.allow_stairs && (culture.stairs || pattern == terraces || cfg.tissue == "terraces")` (V `profiles.rs:87-90`). `cfg.allow_stairs` defaults true (V `config.rs:345`). Cultures with `plan.stairs: true` (V `catalogue.json`, by culture): dwarven, inca, byzantine-greek, native-pueblo, duergar-hold, levantine-tell, himalayan-terraces, newar, al-andalus, italian-comune, caucasian-highland, ethiopian-highland, rajput-fort, tibetan-monastery, maori-pa, latin-american-hillside and others. `medina` has no `stairs` key (V `catalogue.json:129994-130012` has none; U for european-organic and korean, grep only listed the cultures above).
4. Quarter cutting: tries streets at `max_street_grade`; only if no quarter could be cut and stairs are allowed, repeats at `max_stair_grade` and warns `Quarter stairs — streets too steep` (V `macroplan.rs:1372-1392`, `1514-1516`, `warnings.rs:32`). A town no feeder reaches gets its largest quarter served by a level contour spine, the "terrace" of a hill village (V `quarters/service.rs:219-248`).
5. Street roles by steepest segment: over street grade is rank 3; over lane grade is role `stairs` (V `quarters/subdivision.rs:675-690`). Lane traces fall back street, lane, stair grade (V `subdivision.rs:120-145`). Lanes to lots become `stairs` when only the stair grade clears (V `access.rs:225-232`).
6. Terrace tissue: `tissue = "terraces"` sets `contourFollow 1.0` with the organic operator (V `morph.rs:28-31`) and, through `stairs_allowed`, turns stairs on for any culture. Growth then uses grade 0.75 when `programme.slope_relax > 0` (0.5 otherwise) (V `generate.rs:240-248`). Valid tissue values include `terraces` (V `pipeline/validate.rs:94-98`).
7. House slope gate: terraced or stair cultures admit up to tan 30 degrees (0.577) (V `profiles.rs:92,101-105`). `slope_relax` divides the limit by (1 - relax); 1.0 opens it fully (V `profiles.rs:107-110`). It is chosen before the first pass by `slope_relax_ahead` from a terrain survey (V `footprint/survey.rs:84-139`), then by a fallback ladder 0.5, 0.75, 1.0 until half the land is housable (V `profiles.rs:111-116`, `pipeline/ladders.rs:46-90`). Steep ground below that triggers the "slope town" household ladder: densify, relax street gates, terrace wards on the flanks (relax 0.875, 6 m steps, cliff cut at grade 1.25), terraced frontage along roads (V `slopes.rs:1-14`, `142-190`; entry needs `placement == center`, `tissue == culture`, `site_adaptation != 0`, V `slopes.rs:46-51`).
8. Hill-town cultures: the stair cultures above all carry `orientation: terrain`. Terraced morphologies come from the culture's recipe, not from a separate preset system.

### Knobs our wrapper can set

Recipe `[magna_urbis.layout]` is deserialised straight into `LayoutConfig` (camelCase, `deny_unknown_fields`, V `config.rs:9`; wrapper `crates/settle/src/run.rs`, `options()`). Keys that matter:

- `maxStreetGrade` default 0.12, valid 0.02 to 0.3; `maxLaneGrade` default 0.18, valid street to 0.4; `maxStairGrade` default 0.65, valid lane to 1.0 (V `config.rs:342-344`, `pipeline/validate.rs:143-148`). The cap is 1.0, so a 45 degree face (grade 1.0) is the hardest face a stair street can take, and only along the fall line.
- `allowStairs` (default true), `tissue` ("terraces"), `siteAdaptation` (default 1.0, 0 disables the slope ladder), `programme.slopeRelax` (0 to 1; the programme table path is U: field `slope_relax` is in `ProgrammeContext`, V `programme/model.rs:36`), `culture`.
- Our recipe `balmora2-x10s-b2v.toml` has empty `[magna_urbis.layout]` and `[magna_urbis.parcels]` tables (V recipe lines 282-283), so every default above applied, and the cultures were `european-organic`, `medina`, `korean` (V recipe `[cultures]`).

## 3. Does MU run a terrain step we skipped?

No. There is no site conditioning or surface preparation stage in the town pipeline (V `pipeline/mod.rs:100-124`: stilt and shoreline planning surfaces only, then `relax_slopes`). What it does have before quarters is the terrain survey that picks `slope_relax` (V `survey.rs:84`). We already go through it because we call `generate_quarters` (V `run.rs`). The region stage (`settlements/`) may rate sites by slope, but our wrapper replaces it with a fixed centre (U, not read).

## 4. Why our runs failed (hypothesis) and what to do

Reading the facts together (U where marked, since we saved no layout warnings): on the true trench, `european-organic`, `medina` and `korean` are not stair cultures. So streets stop at 0.12 (0.18 lanes); the growth ladder can enlarge the footprint (V `generate.rs:268`) but quarter cutting needs `stairs` to retry at stair grade (V `macroplan.rs:1372-1382`), and the slope-town ladder needs a first layout (U). A 0.5-steep step gave 154 lots, which is consistent with a 0.5 climb being within the lane grade after oblique routing plus relaxed gates (U). Warnings in `layout.warnings` (`Quarter stairs`, `no_quarter_road`, `slope_shortfall`, V `warnings.rs:32,170,391`) name the cause; the wrapper should print them.

### Ranking (all deterministic, MU-native)

1. **Run MU as designed on true terrain with the stair rung on.** In `[magna_urbis.layout]`: `tissue = "terraces"`, `maxStairGrade = 1.0`, `maxLaneGrade = 0.4`, `maxStreetGrade = 0.3` (all inside validate limits), and a stair culture with a medina-like tissue (`al-andalus`, `italian-comune`, `latin-american-hillside`; V catalogue). Then print `layout.warnings` and `slopeRelax`. Cost: one recipe edit. Risk: a street at 0.3 or stairs at 0.65 to 1.0 on the walls need a game-side stair or ramp mesh; MU only labels them (`role stairs`).
2. **Plan on moderated relief only for the grade tests, then re-seat on true heights.** Keep `relief_scale` below 1 as we did (0.25 gave 773 lots) as a fallback; footing records then use plan heights, so rebase `base_z_m` onto true heights at build time (U, our earlier note says they read plan heights).
3. **Our own terrain operator.** Keep only as site conditioning where MU has no answer: it should not terrace the whole valley. MU gives no cut and fill, no walls, no ramps. Reduce `valley_terrace` to (a) a few wide shelves that give the flat quay ground the founding crossing needs, (b) grading of the pads and retaining walls MU lists in `footing.terrace` (so the game honours `base_z_m`, `wall_m`, V `massing.rs:405-420`), and (c) stair or ramp geometry for `role = stairs` streets. The 45 degree steps with 4 lots show the full-valley terracing is counterproductive: it made faces steeper than MU's 0.65 stair grade, since its own plan cannot climb above grade 1.0 (V `validate.rs:148`). Drop the operator as a precondition for planning; keep it as the build-time mesh of MU's own output.

## Unverified

- The cause of 4 lots on the true terrain (no saved warnings or run to confirm).
- Whether `al-andalus` etc. produce a medina-like look in our kit.
- The `programme.slopeRelax` key path through `[magna_urbis.layout]`.
- Behaviour of the stair cultures on a trench (never run).
