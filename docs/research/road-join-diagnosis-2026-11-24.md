# Why the regional roads never join Balmora 2.0 (diagnosis, 2026-11-24)

Inputs: `x10hs-b2w/roads.json`, site `balmora-x10hs-b2w-c1`, recipe `balmora2-x10s-b2w.toml`, layout `target/s1a-streets/medina`.
Scripts: `tools/inspect/roadjoin/` (`mu_grade.mjs` copies MU's grade test on settle's 5 m
crop; `joins.mjs`, `contact.mjs`, `ends.mjs`, `sites_variant.mjs`). Arms were run with copies of the existing exes
(`target/s2d-bin`), outputs in `target/s2d-*`. The baseline reproduces exactly: roads.json is byte-identical, 496 streets.

## 1. How MU ties a town to its regional roads (vendor core, `urban/`)
- `macroplan.rs:1271` clips every imported road to the town envelope. `:1291-1318`: a clipped piece is kept only if
  `routing::path_clear` passes, meaning it is dry and the steepest half-cell grade of every leg (`routing.rs:177-191`)
  is at most `maxStreetGrade` = 0.12. Otherwise `route_within(start, end, 0.12, envelope)` tries to reroute it. If
  that fails, the piece is dropped with "No road connection — steep approach". There is no other fallback.
- `macroplan.rs:1282` + `:1523-1533` add an "approach" street for a town beside a road. It is built only when the
  single nearest road point lies 0.7 r to 2.5 r from the centre, and `planner.route(anchor, nearest street, 0.12)`
  inside the envelope bbox ± 2 cells. If that route fails, nothing is reported.
- `network/exits.rs:392` `ensure_regional_access` (called from `pipeline/finish.rs:83`) is the last resort. If no
  vertex of the largest street component lies within 3 m of a regional road, it routes the 4 nearest street ends to
  the road at `maxLaneGrade` 0.4 (pad 120 m, no wall crossing) and names the result "outbound". It also fails silently.
- "N quarters released — no road access" (`quarters/service.rs:283`) follows from all of the above.
## 2. The 3 roads (baseline; r = 789 m, centre at height 5.5 m; heights and grades on the 5 m crop)
| seg | where its cut end lies | last stretch | gap to the town | what rejects it |
|---|---|---|---|---|
| 27 local | 620 m (0.79 r), height 26 m, inside the envelope | 2 last legs 1.34 and 1.75; 282 of the last 362 m are over 0.12 | 132 m to an approach/3 (mean grade 0.14); 295 m to a gate | path_clear, then route_within: dropped (steep approach #1) |
| 76 track | 472 m (0.60 r), height 39 m, inside the envelope | flat last 90 m (0.01-0.09), then legs of 0.96, 0.66 and 0.39 | 11 m to a path/3, crosses it; 340 m to a gate | same: dropped (steep approach #2) |
| 28 local | 1287 m (1.63 r), height 21 m | legs of 0.82 and 1.40; it climbs 53 m | 731 m | never considered: the approach uses only the nearest point, seg 76 at 472 m, which is below 0.7 r = 552 m |
Result: none of the 3 roads is kept, no approach street is built, and `ensure_regional_access` adds nothing. The
largest component (20.4 km) touches no road within 3 m. The roads' own router works on 118.9 m cells (`grid` 512).
It cuts straight 60 m legs across the valley shoulder, and their steepness only shows at 5 m. Its grade limit is only
a weight (`settlements/roads/mod.rs:1001`), and the steepest grades it reports are 0.48 and 0.39.
## 3. The trim (`crates/roads/src/ends.rs` clip, `run.rs:227-268`)
The roads site `balmora-2` in `x10s-b2-sites.json` is centred at (-2666.4, -1659.8) with radius 900, and it has a
landing at (-2757.0, -1579.6). The settle site is centred at (-2866.4, -2039.8): 429 m from the roads centre and 473 m
from the landing. So the 900 m disc cuts the roads 0.6 to 0.8 r from the town centre, on the shoulder 20-35 m above
the valley floor. These ends are neither "through the town" nor "beside it" in MU's terms. The trim drops 1706 m from
the road that crosses the valley (pieces 27 and 28) and 740 m from the track (76). Those dropped parts are the descent
to the valley floor. So yes, the trim cuts too short and leaves the ends on steep ground. Arms:
| arm (balmora-2 in the roads sites file) | regional streets kept | town touches a road (3 m) | lots |
|---|---|---|---|
| baseline (centre as above, r 900) | 0 | no | 1039 |
| orig-r1 (untrimmed, routed to the landing) | 0 | no, and "No road access — constrained terrain" | 1074 |
| **cocentred-r1** (centre = settle centre, untrimmed) | 1 (29 m, at 430 m from the centre) | **yes** (seg 3: regional/2, market, approach) | 1075 |
| cocentred-r900 / r1000 (ends at 1.14 / 1.27 r) | 0; the approach window is hit, but the route at 0.12 fails silently | no | 1040 |
Passing the road untrimmed to the settle centre, which is MU's native input, restores contact. The road is still dropped
between 430 m and the edge (legs of 0.39, 1.15 and 0.75), so the town is not yet drivable from outside. No data value
alone joins the town: lowering the road trim only moves the end, and raising `maxStreetGrade` changes every town street.
## 4. Proposed fix: smallest generic change, two parts
- **Data (`x10s-b2-sites.mjs` / the site export):** the roads site of a town that MU lays out gets the settle site's
  centre and no disc trim (radius 1, or a "laid-out" flag that skips `ends::clip`). R1b's trim exists for kept towns,
  whose buildings already stand. The guide's copy of the road should be cut at the settled town's claim after settle,
  not at a disc chosen beforehand.
- **Vendored MU patch (`urban/macroplan.rs`):** (a) at `:1316-1318`, a dropped piece leaves an approach anchor where
  it meets the envelope; (b) at `:1282`, `approach` becomes a list: today's nearest-point anchor plus these anchors;
  the bbox at `:1283-1288` grows to cover them all; (c) at `:1524-1533`, each anchor is routed at `max_street_grade`,
  then at `max_lane_grade` (a steep approach stays a track, as the regional profile is only a weight), and the
  "steep approach" warning is kept only when both fail. This is generic (no universe data) and in upstream's style. Add a test next
  to `kinds_camp_mapping_tests.rs`: a road that ends on a 30 % shoulder must still reach a town street.
- **Expected:** segs 27 and 76 (or seg 3 and 32 untrimmed) each get a rank-1 approach street joined to the town; the
  largest component touches the regional network; the "no road access" quarters return. Risk: the planner may find no
  0.12 path inside the bbox. So the first build slice adds `log_warning_detail` to the 3 silent route calls
  (`:1305`, `:1525`, `exits.rs:420`) and reruns cocentred-r1 before writing the patch.
