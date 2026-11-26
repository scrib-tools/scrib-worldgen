# Vendored Magna Urbis

Settlements and roads are Magna Urbis (owner 2026-11-24, `README.md` "Working rules"). This folder is a copy of
its library crates. Every change we need goes **inside this copy**, is generic and data-driven (no universe names,
no Morrowind conventions). The copy is ours (owner 2026-11-24): we never contribute upstream; we take upstream
updates only when they help us.

- Upstream: <https://github.com/dunkean/magna-urbis>
- Rev: `21e22171f0b1766650b2cfef5d4bdaf574a1d325` (upstream HEAD on 2026-11-22; vendored 2026-11-24)
- Licence: AGPL-3.0-only (`LICENSE`, `THIRD_PARTY_NOTICES.md`, `licenses/`, copied unchanged). Our own crates are
  AGPL-3.0-or-later, which links with it.

## Layout

Upstream's relative layout is kept, so an upstream diff applies here with `git apply --directory=vendor/magna-urbis`:

```
vendor/magna-urbis/
  LICENSE  THIRD_PARTY_NOTICES.md  licenses/        upstream root files, unchanged
  rust/Cargo.toml                                   upstream workspace, trimmed (below)
  rust/crates/{core,geom,random,terrain,houses,graph,local}/   unchanged
  rust/fixtures/                                    unchanged (houses/random tests include_bytes! them)
  .gitattributes                                    ours: `* -text`, no eol conversion
  VENDOR.md                                         ours: this file
```

`rust/Cargo.toml` is a nested workspace: the crates inherit `edition`, `license` and `publish` from its
`[workspace.package]`. Our root `Cargo.toml` excludes `vendor/` and points path dependencies at
`rust/crates/{core,geom,terrain}`, so Magna Urbis is not one of our members: our `[workspace.lints]` (pedantic) and
`cargo clippy --workspace` never touch it, and its profiles apply only when it is built on its own (from
`vendor/magna-urbis/rust`). In our builds our root profiles apply (`[profile.dev.package."*"]` = opt-level 3), as they
did for the git dependency. As a path dependency its rustc warnings are no longer capped: `magna-urbis-core`
prints 8 dead-code/visibility warnings at rev 21e2217 (upstream's own; they don't fail `-D warnings`, which
reaches only our members). `.gitattributes` keeps upstream bytes exactly on a `core.autocrlf=true` checkout, so the
JSON catalogues compiled in with `include_str!` never change.

## What was copied (from `git archive` of the rev, ~81 MB, 866 files)

| Path | Size | Why |
|---|---|---|
| `rust/crates/core` | 48 MB (src 37 MB incl. test fixtures and the 3.1 MB catalogue, tests 11 MB) | towns, roads, programme, harbours; used |
| `rust/crates/houses` | 28 MB (tests 18 MB, data 5.7 MB) | architectural engine, for building generation later |
| `rust/crates/terrain` | 1.5 MB | V2 water chain; used by `crates/water` |
| `rust/fixtures` | 1.8 MB | shared fixtures of houses/random tests |
| `rust/crates/{local,graph,geom,random}` | 177 / 133 / 89 / 6 KB | `local` (core + houses), `graph`, path deps |
| `LICENSE`, `THIRD_PARTY_NOTICES.md`, `licenses/` | 3.1 MB | licence terms |

Left out: `rust/crates/{wasm,houses-wasm,neural,neural-native,neural-wasm}` (browser and neural adapters),
`rust/{desktop,desktop-v2,experiments,tools,models,bridge,scripts}`, `rust/Cargo.lock` (its lock covers the left-out
crates; ours resolves the vendored crates), `rust/rust-toolchain.toml` (1.98.0 + wasm target; we build with our own
toolchain), `rust/README.md`, and everything outside `rust/` (web app, assets, Godot renderer, docs, python runtime).
A few Magna Urbis tests and probe examples read `../../../web/...` fixtures at run time; they fail or skip in a
standalone build of this copy. None of them is in our test suite.

## Patch policy

The copy starts byte-identical to the rev (vendoring trims excepted). Every local change:

1. is generic and data-driven, written in upstream's style (so their later updates merge cleanly);
2. touches only files under `vendor/magna-urbis/`, and comes with its test where upstream has one;
3. gets one entry below: number, date, files, what and why.

Keep each patch as its own commit (subject `mu-patch NN: ...`) so `git format-patch` can re-apply it after an update.

### Vendoring trims (not code)

- **T1 2026-11-24** `rust/Cargo.toml`: members cut to the vendored crates (dropped `crates/wasm`, `crates/neural`,
  `crates/houses-wasm`, the `exclude` of `crates/neural-*`) and `[profile.dev.package.magna-urbis-wasm]` removed.
  Why: the left-out crates are not here. Not for upstream.

### Patches

- **01 2026-11-24 culture `hlaalu` (data + one code row). Superseded 2026-11-24 by a culture pack, data reverted.**
  It added a `mediterranean-modern` clone sized for the Hlaalu house kit by editing `catalogue.json`, `activities.json`,
  `enclosures.json`, `harbours.json`, `services.json`, `forms/styles_modern.json`, the `MAPPINGS` row (later
  `culture_mappings.json`) and the culture count in `profiles.rs` (115 -> 116). The culture now lives outside this copy,
  in scrib-worldgen's `recipes/cultures/hlaalu.toml` (every value with its kit source), which settle installs through
  patch 02 (`[culture_packs]` of the recipe). Those files are back to upstream bytes (`cmp` with rev 21e2217), the
  `hlaalu` row is out of `culture_mappings.json` and the count is 115 again. Gate: settle on Balmora 2.0 (recipe
  `balmora2-x10s-b2w`, site c1, Hlaalu kit) gives a byte-identical `medina/lots.json` with the pack.

- **02 2026-11-24 data documents through one registry, overlay packs merged at startup (code).** Why: every
  culture tuning meant editing vendored JSON and rebuilding Magna Urbis (the core test suite alone compiles for
  ~12 min); an application now adapts it through data packs instead (owner rule: adapt Magna Urbis through data).
  - New `rust/crates/core/src/data.rs` (`pub mod data` in `lib.rs`): each data document is the parsed JSON of its
    built-in file under a stable key (`catalogue`, `services`, `activities`, `enclosures`, `harbours`, `styles`,
    `styles_modern`, `roles_core`, `roles_monuments_a`, `roles_monuments_b`, `roles_rural`, `roles_modern`,
    `fortifications`, `urban_admission` (`houses/data/urban-admission.json`), `programme_tokens`, `spring_water`,
    `tunnelling`, `demographic_classes`, `native_witnesses`, `functional_source_coverage`, `culture_mappings`),
    parsed lazily, once. `pub fn install_packs(&[serde_json::Value]) -> Result<(), DataError>` merges overlay
    packs, once, before the first read (`AlreadyRead` otherwise); never called = built-in data as is.
  - A pack is `{ "<document key>": <patch> }`, applied in order. A patch is RFC 7396 JSON Merge Patch plus two
    directives in object position: `{"$from": "<JSON pointer into the same document>", ...rest}` (deep copy of that
    node as the document stands before this patch, then `rest` merged onto it) and `{"$append": [...]}` (appends the
    items an array does not hold yet). Any other `$` key is an error (no built-in document has `$` keys).
  - Every non-test `include_str!` of a data document now reads `crate::data::doc(key)` (a `&'static Value`) or
    `crate::data::typed(key)` (its old typed deserialisation, from the value): `landuse/farms.rs`,
    `settlements/demographics.rs`, `settlements/roads/tunnelling.rs`, `urban/{activities,culture,enclosure,
    farmsteads,feasibility,harbour_furniture,morph,roles,springs}.rs`, `urban/forms/{kit,roles}.rs`,
    `urban/fortification/style.rs`, `urban/parcels/{admission,courts}.rs`, `urban/programme/demand.rs`. The two
    farmstead readers that cut the first `"farmstead"` block out of the catalogue text take `catalogue.farmstead`
    (a test checks it is the same table). Test-only includes are unchanged.
  - `urban/culture.rs` `MAPPINGS` (the culture -> architecture tuples) became the
    built-in document `urban/culture_mappings.json`: `cultures.<id> = {family, pack, borrowed, functionalProfile,
    architectureFamily}` (land family, houses pack, declared borrowing, what a lot holds, architectural layout), read
    by `mapping_rows()` / `mapping_row()`. `mappings()` now lists cultures in id order (used only by a count test).
    `PREFERRED` stays code.
  - Tests: `data::tests` (merge, `$from`, `$append`, errors, farmstead table) and `urban/culture_pack_tests.rs`
    (a pack clones `mediterranean-modern` into a new culture with `$from` for the culture, its presets, farmstead, mapping and
    styles, and `$append` to its family lists; the culture checks and a small town pass, in a child process because
    packs install once per process). Behaviour unchanged: on the settle gate (Balmora 2.0, recipe `balmora2-x10s-b2w`, site c1, Hlaalu kit)
    `medina/lots.json` is byte-identical and `medina/layout.json` differs only in its echo of the recipe's `fit` block
    (changed by settle in the meantime).
  - Re-apply after an upstream update: copy `data.rs` and `culture_pack_tests.rs`; add any new data `include_str!`
    to `DOCUMENTS` and point its reader at `doc`/`typed`; regenerate `culture_mappings.json` from upstream's
    `MAPPINGS` (one object per tuple: fields in tuple order culture, family, pack, borrowed, functionalProfile,
    architectureFamily) and replace `MAPPINGS` with `mapping_rows()`/`mapping_row()` again.

- **03 2026-11-24 preset param `minFrontage`: a culture's smallest street front (code).** Why: a lot's minimum front
  was only the borrowed houses recipe's witness (`min_envelope_m[0] + 0.6`, ~5.6 m for EUR-MED), whatever the
  culture's own building kit holds; `frontage` scaled by density (`morph.rs:137`) and pressure could not lift it, so
  a pack sized for wider houses still got ~6 m lots (Balmora 2.0: 84 street-front dwelling lots under 8 m, min 5.9 m)
  whose MU house took the front's width. No data value changed that.
  - `urban/parcels/admission.rs`: new `with_min_frontage(a, m)`: raises a tier's `min_frontage` to `m` and its
    `min_area` to at least `m * min_depth`; no-op when `m` is 0, absent, or not above the tier's own front.
  - `urban/parcels/block_preparation.rs` (where each block's admission and compact fallback are built): reads
    `minFrontage` (metres, not density-scaled) from the block's morph and floors both tiers with it; the fallback is
    still chosen from the unfloored primary. Every existing consumer then honours it: the `_witness` that `strips`
    fits widths to, `pressure::apply`'s frontage floor (1.15x), `qualify`'s envelopes and merge/rebalance repairs,
    and `seat`'s front rule. Unmapped cultures (no admission, block_cut's fixture envelope) are not affected.
  - Tests: `urban/parcels/min_frontage_tests.rs` (a burgage block of `mediterranean-modern`: with 8.4 m no `plot` has a
    front under it; absent, 0 and 1.0 give the same JSON). Gate: settle on Balmora 2.0 (recipe `balmora2-x10s-b2w`,
    site c1, Hlaalu kit) with the pack's `minFrontage` removed gives a byte-identical `medina/lots.json`.
  - Use: scrib-worldgen `recipes/cultures/hlaalu.toml` sets 8.4 m on every hlaalu preset. Not in catalogue
    `fields.parcels` (a preset value, not a user override; a pack may `$append` it there).
  - Re-apply after an upstream update: copy `with_min_frontage` and the test file (`mod min_frontage_tests` in
    `parcels.rs`), and wrap the admission/fallback construction in `block_preparation.rs` again.

- **04 2026-11-27 routing barrier: an optional mask that water goes round (code). In three commits; 1 of 3 here.**
  Why: an application that keeps places of its own on the relief (a hand-made town on generated ground) needs the
  courses to spill round them. Raising the input instead does not work: the valley cut aims at an absolute section
  (`vector_water/incision.rs`), so a raised wall becomes a deep cut in the band around it, and a raised routing-only
  surface leaves wall-high `filled` levels that `rivers/support.rs` (`across_beds`, `floor_path`) and the line levels
  read as water. So the mask goes inside the flood. `None` gives today's bytes everywhere.
  - Commit 1, `priority_flood.rs`: `flood` takes `barrier: Option<&[u8]>` (non-zero = barrier). A barrier cell is
    marked `BLOCKED` (-3) after the sea fill, so `reach` never queues, fills or expands it; it is never an outlet
    (map edge and `outlet` flag included); a flagged sea cell stays sea. D8 runs with `filled` = infinity on barrier
    cells (it ignores the flood parent wherever the ground drops), restored to the cell's height after. Barrier cells
    and any cell they cut off end `order` in raster order (so `order` still lists every cell once), hold receiver -1
    and `filled` = their own height; a barrier cell's accumulation is 0. Entries: `flood_grids_masked`,
    `routing_grids_masked` (`pub(crate)`); `flood_grids` and `routing_grids` call them with `None`.
  - Tests: `priority_flood/tests.rs`, four: `None` and an all-zero mask against `flood_grids` / `routing_grids` bit
    for bit on the reference's grids; a tilted 9 x 9 plane with a 3-cell wall (every receiver and area by hand); a
    ringed pit (stays unreached, not filled); a barrier on the map edge and on the sea.
  - Re-apply after an upstream update: the five guarded blocks in `flood` (mark, the `queue` test, the tail of
    `order`, infinity around D8, the accumulation start), the parameter on its five callers, the two `_masked`
    entries, and the tests with their helpers (`same_grids`, `slope`, `barrier_on`, `each_cell_once`).

## Updating to a newer upstream rev

1. Note the new rev; read upstream's log between the two revs for the crates we use.
2. Export it fresh into an empty folder, same paths as above:
   `git -c core.autocrlf=false archive --format=tar <rev> rust/crates/{core,geom,random,terrain,houses,graph,local}
   rust/fixtures rust/Cargo.toml LICENSE THIRD_PARTY_NOTICES.md licenses | tar -x -C <new>`
   (run inside a clone of upstream; check that any new path dependency or `include_*!` target outside the copied
   crates is added to the list).
3. Re-apply trim T1, then each patch in order (`git format-patch` of the `mu-patch` commits, `git apply
   --directory=vendor/magna-urbis`); drop a patch only when the update makes it unneeded, and say so here.
4. Replace `vendor/magna-urbis/` with the new folder (keep this file and `.gitattributes`), update the rev here and
   in the root `Cargo.toml` comment.
5. Verify: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and a `settle` run on
   a known site compared with the previous output (differences must be explained by the upstream log).
