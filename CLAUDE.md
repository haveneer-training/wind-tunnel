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

Which doc is which:

| File | Audience / content |
|---|---|
| `docs/AVANCEMENT.md` | handoff doc — state, remaining work, conventions, rationale. The authority |
| `README.md` | trainee entry point: what the project is, the `starter` → `goto` → `solve` loop |
| `ETAPES.md` | the 11 steps + bonuses in one page, trainee-facing |
| `docs/etapes/etape-NN.md` | one step's statement: mandatory core + optional extension |
| `docs/BONUS-OPTIMISATION.md` | the allocation thread — measurements and the solutions to steps 7/8's "pour aller plus loin" |
| `docs/README-travail.md` | `make_starter` writes it as `travail/README.md`, and skips it when copying `docs/` |

## Commands

```shell
cargo test                                              # all tests (79 currently)
cargo test --test mesh                                  # one test file
cargo test the_mesh_is_mixed                             # one test by name
cargo clippy --all-targets --all-features -- -D warnings # must stay clean
cargo fmt --all                                          # / --check to verify only
cargo run --release -- domains/tunnel.dom --refine 4 --bands 9 --steps 960
cargo run --release --example alloc_count                # allocations per phase (see docs/BONUS-OPTIMISATION.md)
```

xtask (drives the step-by-step exercise machinery, used from the repo root on this corrigé,
and from `travail/` by trainees; `cargo xtask` is an alias defined in `.cargo/config.toml`):

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
cargo xtask goto 12 && cargo xtask solve 12                # ... and back to green

scripts/check-steps.sh   # same round-trip, automated, step by step from 0 to LAST_STEP
```

Step 11 (MPI, bonus) lives in the `mpi/` crate. It **is** a workspace member (shared
`Cargo.lock`, visible to `cargo metadata`/rust-analyzer), but not a *default* one — a
non-virtual workspace only builds its root package by default, so plain `cargo build`,
`cargo test`, `cargo clippy --all-features` (no `-p`/`--workspace`) never touch it and stay
green on a machine without MPI. To build and run it:

```shell
cargo build --release -p wind-tunnel-mpi
mpirun -n 4 target/release/wind-tunnel-mpi domains/tunnel.dom --refine 4 --steps 200
scripts/check-mpi.sh     # 1/2/3/4 ranks vs. the sequential binary; exits 0 if no mpirun
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
| `src/velocity.rs` | analytic carrier flows (uniform, potential flow around a cylinder) + the `StreamSource` contract the solver actually consumes |
| `src/stream.rs` | the computed flow (step 12): `∇²ψ = 0` solved at the vertices, dual-mesh weights in CSR, Jacobi sweeps |
| `src/flux.rs` | flux schemes behind the `FluxScheme` trait: `Upwind`, `Centered`, `Muscl` (order 2, step 10) |
| `src/gradient.rs` | least-squares gradient reconstruction per cell + Barth–Jespersen limiter — what `Muscl` extrapolates with (step 10) |
| `src/solver.rs` | time-stepping loop, CFL, boundary conditions |
| `src/io/` | VTK and PNG output |
| `src/error.rs` | the two error families (mesh-time vs. solver-time) |
| `src/app.rs` | CLI args and case assembly, shared by both binaries |
| `src/decomposition.rs` | vertical band decomposition for MPI (step 11, feature-gated) |
| `mpi/` | the MPI driver — a separate crate, a workspace member but not a default one (step 11; see above) |
| `xtask/` | generates `travail/` and drives step progression (see Commands) |

### Load-bearing design decisions (do not casually change — see `docs/AVANCEMENT.md`)

- **Face fluxes come from a stream-function difference**, not from sampling velocity at
  faces. This is what makes the discrete divergence exactly zero (machine precision) and
  keeps the upwind scheme bounded. An earlier velocity-sampling version let a field in [0,1]
  drift to 1.78 — two tests lock this property in.
- **Conservation does not depend on the flow's quality.** A face flux is `ψ(b) − ψ(a)`, so a
  closed cell's balance telescopes to exactly zero for *any* nodal ψ — converged or not,
  physical or not. This is why step 12 could not endanger the two `1e-12` tests, and it is the
  project's sharpest teaching point: the property comes from the shape of the scheme.
- **Walls use `ZeroGradient`, not `NoFlux`** — *with the analytic flow*. A staircase wall isn't
  exactly a streamline of it; forcing zero flux would fabricate an artificial divergence and
  break the boundedness. With `--flow computed` (step 12) the residual is zero by construction,
  and `app::solver_config` switches `Wall`/`Obstacle` to `NoFlux` there. Don't generalize that
  switch to the analytic flow.
- **Typed indices** (`CellId`, `FaceId`, `VertexId`) instead of pointers, and **`Side`**
  instead of an `Option` + separate flag — deliberate teaching choices as well as technical
  ones.
- **The time loop is written in *gather* form**, not *scatter* — this is what makes it
  parallelizable with `rayon` (step 9) without restructuring, and is used as the
  C/OpenMP-style pitfall example.
- `obstacle()` in `src/app.rs` reduces the drawn shape to an equivalent-area disk, and
  `PotentialCylinder` flows around that disk — the analytic flow does not "see" the actual drawn
  shape. Fixed by `--flow computed` (step 12, demo: `domains/square.dom`), but analytic is still
  the default, so the pitfall is still live.
- **`Solver::new` is generic over `StreamSource + ?Sized`**, which is what let step 12 add a
  vertex-indexed ψ field without touching any of the twenty existing call sites. A
  `&dyn VelocityField` cannot be coerced to `&dyn StreamSource`, hence `app::Carrier` carries the
  impl and dispatches. `ComputedStream` deliberately does not implement `VelocityField`.
- **`faer` is an optional dependency**, used only by `examples/stream_faer.rs`
  (`cargo run --release --features faer --example stream_faer`). `cargo test` never compiles it.
  Don't promote it to a normal dependency: the trainee's inner loop is `cargo test`.
- **VTK frames carry more than the tracer**: `write_frame` adds cell velocity `u`/`speed`
  (reconstructed from the face fluxes by `Solver::velocity_at`, no `VelocityField::at`
  involved, so it works for a computed flow too), vertex `psi`, and the frame's date. The
  step-3 hole stays exactly what it was — it is now `write_dataset`, writing the geometry
  and cell scalars into a `&mut impl Write`; `write_vtk` and `write_frame` are the
  non-holed wrappers around it.
- **`--frame-dt` outputs at fixed physical time**, which is what makes two runs comparable
  frame by frame: different flows get different CFL-imposed `dt`, so equal frame indices
  are *not* equal instants. Implemented in `Solver::run` (`Config::output_dt`), hence
  refused by the MPI driver, whose own time loop is the step-11 hole.
- A `.pvd` collection would be the clean way to carry time into ParaView, and it is
  deliberately **not** used: it crashes `vtkPVDReader` 6.1.1 on legacy VTK (tested — see
  the comment in `mpi/src/main.rs`). The date is written as `TIME`/`TimeValue`/`CYCLE`
  field data instead, and `--frame-dt` is the reader-independent fallback.
- **`VtkF64` in `src/io/vtk.rs` writes denormals as `0`** (and switches to `{:e}`
  outside the usual exponent range). Not cosmetic: VTK's legacy ASCII reader parses with
  `istream >> double`, which sets `failbit` on underflow, then abandons the rest of the file
  — ParaView shows "Unsupported cell attribute type" and displays garbage. The tracer decays
  below `f64::MIN_POSITIVE` within a few dozen steps, so this hits every real run. One test
  locks it in. It is a `Display` type, not a `fn(f64) -> String`: a frame holds ~10⁶ numbers,
  and one `String` each cost 273 691 allocations per file against 3 now.
- **Allocation discipline is a teaching thread of its own**, written up in
  `docs/BONUS-OPTIMISATION.md` and measured by `examples/alloc_count.rs` (a counting
  `GlobalAlloc`). The time-loop buffers `residual`/`step_rk2` still allocate are deliberate
  — they are the "pour aller plus loin" of steps 7 and 8, and the bonus doc holds their
  solution. Output paths (`vtk.rs`, `png.rs`) and the MPI `Halo` buffers are already fixed:
  do not reintroduce per-cell or per-value allocations there.

### Step machinery (feature-gated exercises)

Each training step is a Cargo feature (`step0` … `step12` currently, chained: `stepN = ["stepN-1"]`),
so `cargo test` in `travail/` only shows tests for steps already opened. `LAST_STEP` in
`xtask/src/main.rs` must track the highest implemented step. In this corrigé, source blocks
between `// SOLUTION-BEGIN` / `// SOLUTION-END` (preceded by a `// TODO-STEP:<n>` comment)
mark what `xtask starter` turns into a `todo!()` in `travail/`; test files for step N are
gated with `#[cfg(all(test, feature = "stepN"))]` or `#![cfg(feature = "stepN")]`.

Conventions when adding a new step (full list in `docs/AVANCEMENT.md`):
1. One hole = one `SOLUTION-BEGIN`/`SOLUTION-END` block covering a whole function body or
   expression, preceded by `// TODO-STEP:<n>`; it must typecheck with `todo!()` in its place.
2. Step N's tests live under the `stepN` feature gate.
3. Silence the warnings the hole causes, with a step-conditional attribute:
   `#[cfg_attr(not(feature = "stepN"), allow(unused_variables))] // trou étape N` on the
   holed item (visible during step N, where it names what is left to write), and
   `not(feature = "stepN+1")` for collateral `dead_code` on untouched helpers (never
   visible). When the warning comes from a `#[cfg]` rather than a hole, gate the import
   instead of allowing it. Full rationale in `docs/AVANCEMENT.md` § "Avertissements dans
   travail/".
   **`step13` is the sentinel feature**: one step past `LAST_STEP`, gating no code, on by
   default in both `Cargo.toml` and `mpi/Cargo.toml` so that every `not(feature =
   "stepN+1")` guard is inert in this corrigé. `goto` rewrites the root manifest;
   `make_starter` clears `mpi/Cargo.toml`'s default, so `travail/` never has it. A step
   that becomes the new last one must move the sentinel up.
4. Bump `LAST_STEP` in `xtask/src/main.rs`. A step that adds a separate crate must also
   add it to `make_starter`'s copy list **and** to `SOURCE_DIRS`, or xtask never sees its
   holes. The `// TODO-STEP:<n>` marker must sit within 8 lines above `// SOLUTION-BEGIN`
   — `step_of` looks no further, and silently attributes the block to step 0.
5. Add `docs/etapes/etape-NN.md` — a short mandatory core, an optional extension.
6. **Language convention**: identifiers and filenames in English; prose (comments, doc
   comments, error messages, exercise statements) in French. Test names are code → English.
7. Regenerate and check: `cargo xtask starter --force && cd travail && cargo test`.

`travail/` is gitignored and excluded from the workspace (`Cargo.toml` `[workspace] exclude`)
— it's a full standalone project a trainee can `git init` themselves.
