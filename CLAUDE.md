# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

"Soufflerie numérique" (numerical wind tunnel) — the fil rouge (through-line) project for a
3-day Rust training ("Découverte du langage Rust pour le calcul scientifique", ONERA). A
finite-volume solver on an unstructured mesh: reads an ASCII domain mask, builds a mixed
triangle/quad mesh, advects a passive tracer around an obstacle, writes VTK + PNG frames.

**This repo is the corrigé (reference solution)** — the complete, working code. Trainees
generate their own `travail/` working copy from it with holes (`todo!()`) to fill in.
`docs/AVANCEMENT.md` is the up-to-date handoff doc: project state, what's left, decisions not
to undo. Read it before making non-trivial changes.

## Commands

```shell
cargo test                                              # all tests (54 currently)
cargo test --test mesh                                  # one test file
cargo test the_mesh_is_mixed                             # one test by name
cargo clippy --all-targets --all-features -- -D warnings # must stay clean
cargo fmt --all                                          # / --check to verify only
cargo run --release -- domains/tunnel.dom --refine 4 --bands 9 --steps 960
```

xtask (drives the step-by-step exercise machinery, used from the repo root on this corrigé,
and from `travail/` by trainees):

```shell
cargo xtask starter [--force]   # generate travail/ (holes punched in, from the repo root)
cargo xtask status              # which steps are done, from travail/
cargo xtask goto <n>            # open step n (fills earlier steps' empty holes, sets default feature)
cargo xtask solve <n>           # fill step n's holes for the trainee
cargo xtask reset <n>           # reopen step n's holes
```

Full check before considering anything done (mirrors `docs/AVANCEMENT.md` §"Vérifier que tout
va bien"):

```shell
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo xtask starter --force && cd travail && cargo test   # 4 red tests expected: step 0
cargo xtask goto 5 && cargo xtask solve 5                  # ... and back to green

scripts/check-steps.sh   # same round-trip, automated, step by step from 0 to LAST_STEP
```

## Architecture

Data flow, matching module boundaries:

```
mask ──▶ mesh ──▶ field ──▶ solver ──▶ io (VTK / PNG)
                    ▲          ▲
               velocity      flux
```

| Module | Role |
|---|---|
| `src/geom.rs` | points, vectors, areas, centroids |
| `src/mask.rs` | domain mask parsing/validation (`.` fluid, `#` solid, `%` comment) |
| `src/mesh.rs` | cells, faces, connectivity — one cell per fluid cell of the mask, 45° chamfers where perpendicular walls meet, giving a genuinely mixed mesh |
| `src/field.rs` | a scalar field over cells |
| `src/velocity.rs` | carrier flows (analytic, e.g. potential flow around a cylinder) |
| `src/flux.rs` | flux schemes (upwind, centered) |
| `src/solver.rs` | time-stepping loop, CFL, boundary conditions |
| `src/io/` | VTK and PNG output |
| `src/error.rs` | the two error families (mesh-time vs. solver-time) |
| `xtask/` | generates `travail/` and drives step progression (see Commands) |

### Load-bearing design decisions (do not casually change — see `docs/AVANCEMENT.md`)

- **Face fluxes come from a stream-function difference**, not from sampling velocity at
  faces. This is what makes the discrete divergence exactly zero (machine precision) and
  keeps the upwind scheme bounded. An earlier velocity-sampling version let a field in [0,1]
  drift to 1.78 — two tests lock this property in.
- **Walls use `ZeroGradient`, not `NoFlux`.** A staircase wall isn't exactly a streamline of
  the analytic flow; forcing zero flux would fabricate an artificial divergence and break the
  boundedness. Step 11 (bonus) removes the residual at the source instead.
- **Typed indices** (`CellId`, `FaceId`, `VertexId`) instead of pointers, and **`Side`**
  instead of an `Option` + separate flag — deliberate teaching choices as well as technical
  ones.
- **The time loop is written in *gather* form**, not *scatter* — this is what makes it
  parallelizable with `rayon` (step 9) without restructuring, and is used as the
  C/OpenMP-style pitfall example.
- `obstacle()` in `src/main.rs` reduces the drawn shape to an equivalent-area disk, and
  `PotentialCylinder` flows around that disk — the flow does not "see" the actual drawn shape
  (fixed in bonus step 11, not yet implemented).

### Step machinery (feature-gated exercises)

Each training step is a Cargo feature (`step0` … `step5` currently, chained: `stepN = ["stepN-1"]`),
so `cargo test` in `travail/` only shows tests for steps already opened. `LAST_STEP` in
`xtask/src/main.rs` must track the highest implemented step. In this corrigé, source blocks
between `// SOLUTION-BEGIN` / `// SOLUTION-END` (preceded by a `// TODO-STEP:<n>` comment)
mark what `xtask starter` turns into a `todo!()` in `travail/`; test files for step N are
gated with `#[cfg(all(test, feature = "stepN"))]` or `#![cfg(feature = "stepN")]`.

Conventions when adding a new step (full list in `docs/AVANCEMENT.md`):
1. One hole = one `SOLUTION-BEGIN`/`SOLUTION-END` block covering a whole function body or
   expression, preceded by `// TODO-STEP:<n>`; it must typecheck with `todo!()` in its place.
2. Step N's tests live under the `stepN` feature gate.
3. Bump `LAST_STEP` in `xtask/src/main.rs`.
4. Add `docs/etapes/etape-NN.md` — a short mandatory core, an optional extension.
5. **Language convention**: identifiers and filenames in English; prose (comments, doc
   comments, error messages, exercise statements) in French. Test names are code → English.
6. Regenerate and check: `cargo xtask starter --force && cd travail && cargo test`.

`travail/` is gitignored and excluded from the workspace (`Cargo.toml` `[workspace] exclude`)
— it's a full standalone project a trainee can `git init` themselves.
