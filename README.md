# scrib-worldgen

Part of [Scrib tools](https://github.com/scrib-tools): offline world generation for
[Scrib](https://github.com/scrib-tools/Scrib) and any engine that reads its files. Generates nature between kept
human-made content and grows or founds settlements. It talks to Scrib through files only.

**Licence: AGPL-3.0-or-later** (`LICENSE`, owner 2026-11-23), so it can build on
[Magna Urbis](https://github.com/dunkean/magna-urbis) (AGPL-3.0-only, Rust). Scrib itself stays GPL-3.0-or-later and
links none of this code. What you generate with it (worlds, layouts, placements) is your data.

Status: settlements spike (`docs/specs/settlements.md`). The terrain generator (`sf_worldgen`, `mw_worldgen` in Scrib)
moves here later with its history.

## Working rules

Scrib's [CONTRIBUTING.md](https://github.com/scrib-tools/Scrib/blob/main/CONTRIBUTING.md) applies here (research
first, reuse before building, industry standard, tunables declared, one deliverable per slice), except the licence.
On the owner's machine every compiling cargo command goes through Scrib's queue, from this folder:
`bash /d/Gamedev/Bevyrrowind/bevyrrowind/tools/cargo_turn.sh <cargo args>`.

**Settlements and roads are Magna Urbis, vendored (owner 2026-11-24).** Generating cities, towns, hamlets and roads
is a solved problem; Magna Urbis solves it better than we would. We never write bespoke generation logic for one
universe (Morrowind now, others later): no own packers, fillers or layout passes. Each universe is a data adaptation
of Magna Urbis (cultures, catalogue, presets, building kits); a missing behaviour is added inside the vendored Magna
Urbis, generic and data-driven. The copy is ours: we never contribute upstream (owner 2026-11-24); we take their
updates only when they help us. Use every Magna Urbis feature (fortifications,
enclosures, massing, programme, harbours, countryside, regional placement, names, edits, cultures) before writing
anything: rebuilding one of them is reinventing the wheel. The tool serves any fantasy world, not only Morrowind
(building generation comes later, from Magna Urbis's `houses` engine first).
