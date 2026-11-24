# Why the `match` fit leaves plot lots empty (diagnosis, 2026-11-24)

Run `target/c5b-match` (site `balmora-x10hs-b2w-c1`, kit `hlaalu-bs`, recipe `balmora2-x10s-b2w`), first pass of
`crates/settle/src/fit.rs` (`fit_match`, `Pass::best_fit`, `Fitter::try_turn`). Measured, no fix.

## Method

- A node port of the match pass (`try_turn` / `try_candidate` / `best_fit` / `inside` / `pad`, the site's bilinear
  ground, the same recipe tunables) reproduces the Rust stats: 613 parts filled (Rust 614), part failures
  inside 326 / overlap 6 / pad 67 (Rust 326 / 5 / 67), 570 of 903 lots filled (Rust 570). The difference is the seeded
  draw (sha256 in place of blake3). Scripts: scratchpad `fitdiag.js` (port + variants), `diag.js` (per-part
  diagnostics), `buckets.js`, `attr.js`, `v3.js`.
- For each failing part: (a) the smallest worst-point excursion out of the lot over every candidate, door turn and
  slide, and the lot edge crossed (street / side / back by its outward normal against `n`/`u`, as `LotRule`; `notch` =
  a concave lot vertex inside the hull); (b) angle of the part's min-area box to the street frame (0-45°);
  (c) a 0.25 m raster of the lot in the street frame: largest inscribed rectangle, and whether some model's turned
  hull fits anywhere in the lot door-to-street (exact `inside` check on the raster hit); (d) the same with every
  door and with ±90° turns; (e) the part's front vs the lot's street edge at the anchor `u`; (f) pad numbers.
- Gains: full sequential passes (overlap with earlier lots included), each change as a **fallback stage** tried only
  when the baseline finds no candidate, so filled parts keep their placement (1 of 613 lost in every arm).

## Buckets (dominant cause per failing part; empty lots by their part's most recoverable bucket)

| Bucket | Parts | Empty lots | Numbers (p50) | Example lots (`parcel-…#part`) |
|---|---|---|---|---|
| 1 Kit bigger than the lot (every hull area > lot area) | 82 | 57 | lot u-width 4.9 m, part 4.8 × 4.1 m | 07cfcb25#0, 5b597b95#0, 82a1962b#0 |
| 2 No street-frame rectangle for any front (inscribed rect ≥ 7.8 m deep is < 8.0 m wide, or ≥ 8 m wide is < 7.8 m deep) | 83 | 80 | widest rect at house depth 7.0 m | 383f648f#0, a35472c6#0, c89da9bb#0 |
| 3a Lot holds a model only turned 90° (door along the street) | 32 | 29 | widest rect 8.75 m, 11.75 m deep | 5731c64e#0, d46c733d#0, 9420d0c4#0 |
| 3b Lot shape holds no model door-to-street (rect just wide enough, hull doesn't fit) | 26 | 23 | rect 8.5 × 8.75 m, angle 9° | 5f64e7c4#0, cad9f2b9#0, 201bc817#0 |
| 4s Anchored hull crosses the **street** edge (a fit exists elsewhere in the lot) | 38 | 33 | excursion 0.19 m, angle 6°, part front on the street edge (gap 0.0) | 25e2b511#0, 3bb779b0#0, d0062a0a#0 |
| 4n Concave lot corner pokes into the hull (notch) | 21 | 18 | 0.23 m | cf486523#0, 584e4991#0, 7eba085e#0 |
| 4i Crosses a **side** edge | 33 | 30 | 0.42 m, angle 1° | cb38ea26#0, df4a858b#0, 0c0de4f3#0 |
| 4b Crosses the **back** edge | 9 | 4 | 0.60 m; part front 14.7 m behind the street | 9b2532cc#1, 148a24ec#1, b03ba2fa#0 |
| other inside (excursion 0, tolerance edge) | 2 | 1 | | 9c8ec110#1 |
| pad_step | 67 | 58 | 65 fail the **rise** (ground above pad), 2 the drop | 16677bf2#0, 42cf4577#0, 7ed35544#0 |
| overlap | 6 | 0 | | 27487a46#1, e99db262#1 |

- Buckets 1 + 2 (165 parts, 137 empty lots) are the kit-too-big case: model fronts are 8.0-23.7 m (p50 ≈ 14 m),
  depths 7.8-15.6 m, while Magna Urbis footprints are p50 10.5 × 9.3 m. This is wider than "frontage narrower than
  every kit front" (50 lots): many lots have a wide access but narrow below the house depth. No fit rule recovers
  them (≤ 6 parts in any arm); they need smaller kit models or the merge pass.
- (b) The part box is within 1° of the street frame for 184 of 326 inside failures; ≥ 10° for 61.
- (c) In 103 inside failures a door-to-street position exists in the lot; the anchored rule misses it.
- (d) fit.rs tries the first door, plus a back door within 45° of its opposite. Trying every recorded door: +1 part.
  Kit doors are front/back already. A ±90° turn (door along the street, facing a neighbour) is an upper bound only:
  +111 parts but 24 current placements change, and it is valid only where that side is a street (corner lots).
- (e) Part front on the lot's street edge (|gap| ≤ 0.05 m) for 194 of 326. Street crossings happen with the front
  flush: the hull's door side is set on the part's furthest point along the access normal, and an oblique or bent
  street edge (p50 6° off) puts one front corner 0.1-1 m into the street.
- (f) Pad: rise past `pad_rise_max_m` 1.0 m p50 1.69 m (p90 3.25), ground range under the hull p50 2.4 m, door probe
  0.79 m below the hull-centroid ground (the lot rises from the street). Drop below `foundation_depth_m` (2.2-3.2 m):
  2 parts. In 62 of 67 the range fits within rise + foundation depth, so a pad level exists.
- Aside: b_09, b_10, b_21, b_23 and b_27 have a first door at 244-301°, so door-to-street turns their hull 15-30° off
  the frame (street-frame box 16-17 × 12-13 m).

## Smallest generic changes (measured, one at a time as fallback stages, then together)

| Change (fallback after the baseline finds nothing) | Parts | Lots | Recovers |
|---|---|---|---|
| A. Slide the hull back along `n` (0.5 m steps to 6 m; or reuse the lot domain's `Target::depth` street-edge clamp, then step back) | +84 | +70 | 4s 34/38, 4n 17/21, 4i 9, pad 16, overlap 5. Moved back p50 1 m, p90 4 m; capped at 2 m: +74 / +62 |
| B. A + the part's box frame when it is within 15° of the street normal | +107 | +93 | + 3a 9, 3b 9 (23 turned, p50 9°) |
| C. Fallback `pad_rise_max_m` 2 m (back wall buried up to 2 m) | +42 | +36 | pad 42/67 (3 m: +53). Same count as raising the pad to the ground with a door step ≤ 1 m (+40, step p50 0.41 m) |
| A + B + C | **+146** | **+130** | 570 → 700 lots filled. Left: 229 inside (165 kit-too-big), 21 pad, 3 overlap |
| Not worth it: wider u slide (±20 m) | +2 | +1 | the u range is not the limit |
| Not worth it: every door as street door | +1 | +1 | |
| Not as a rule: part frame in place of the street frame | -213 | -196 | doors off the street, more corners out; only as a capped fallback (B) |

- A is the clear first slice: generic, keeps the door to the street, and only moves houses that fail today. Use the
  existing `Target::depth` clamp with a back-step limit tunable (2-6 m). The step cap and C (a buried back wall,
  or a raised door step) change the look, so they need the owner's eye.
- Buckets 1-2 and 3a stay for the kit (narrower models, ≤ 8 m fronts) or the merge / filler passes.
