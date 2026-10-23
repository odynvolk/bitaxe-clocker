# Plan

> Status: executing

## Problem summary

A code-quality review of `bitaxe-clocker` surfaced 24 findings across six tiers. None are
feature requests; all are correctness-of-tooling, structure, idiom, and test-integrity issues.

Requirements were provided inline in-conversation (the findings list below), and the user
approved all six tiers, then explicitly narrowed scope to exclude CI.

Evidence gathered during review (all reproduced by running tools):

| # | Finding | Evidence |
|---|---|---|
| 1 | `.clippy.toml` is entirely invalid; `cargo clippy` hard-fails | 14 `unknown field` errors, build aborts |
| 2 | `.rustfmt.toml` has 3 nightly-only keys silently ignored on stable + 6 no-ops | `--config struct_lit_width=1 --check` → zero diff |
| 3 | Real clippy output is only 3 warnings | `common.rs:47`, `elpriset_just_nu.rs:45`, `main.rs:34` |
| 4 | Toolchain drift | `.tool-versions`=1.90.0 vs `Dockerfile` `rust:1.85-slim` |
| 5 | `Cargo.lock` gitignored for a binary | `git ls-files` → not tracked |
| 6 | `CONFIG` global via 8 × `CONFIG.get().unwrap()`; `_config` bound but unused | `grep -c` per file |
| 7-10 | Three overlapping error strategies; `From<reqwest::Error>` drops the source; ~60 lines of hand-written boilerplate; `factory` silently defaults on one failure but `.expect()` panics on another. **Correction at U3:** the "dead `PartialEq`" claim was wrong — 13 tests compare errors via `assert_eq!`, which the original `==` grep could not match. `PartialEq` is load-bearing; removing it requires converting those 13 sites to `matches!` first. | read + grep |
| 11-16 | `-1` sentinel; `== 200` vs `== StatusCode::OK`; hand-rolled `Pin<Box<dyn Future>>`; mode naming (`default` = two meanings); `ten_millis` holds minutes; i32 `try_into().unwrap()` in loop; `logger` wrapper forces `&format!` at 20 sites | read + grep |
| 17-20 | **0 `.assert()` calls across 4 mockito mocks**; `tests/common.rs` dead; 18/29 tests on two parse fns; `tests/tests.rs` is a shim | grep |
| 21-23 | `COPY . .` kills dep caching; stray tzdata layer; no `--locked`/`USER` | read |

## Relevant learnings

None. `docs/solutions/` does not exist and no prior artifacts match.

## Scope boundaries

**In scope** — all six tiers, minus CI:

- valid `.clippy.toml` / `.rustfmt.toml`; `rust-toolchain.toml`; track `Cargo.lock`
- mockito `.assert()` calls; delete dead `tests/common.rs`
- `thiserror` for both error enums; restore error source chains; `factory` → `Result`
- remove the `CONFIG` static; thread config explicitly
- `Option<i32>` over `-1`; `StatusCode` consistency; naming fixes; typed interval;
  drop the `logger` string wrapper for structured `tracing`
- Dockerfile layer caching, `--locked`, non-root `USER`

**Out of scope** — explicitly excluded or deliberately deferred:

- **CI / `.github/`** — user-excluded. Consequence: the commands in *Verification strategy*
  are the gate, run manually. No justfile/Makefile added (not requested).
- **Behaviour changes.** This is a no-behaviour-change refactor. Notably NOT touched:
  the cheap/default/expensive threshold *values*, the restart-on-change policy,
  the price-provider fallback semantics, the serial (non-concurrent) device loop.
- **`check_interval` drift fix** (finding 16's "sleep after work" observation) — deferred;
  changes runtime timing behaviour. Only the *typing* of the interval is fixed here.
- **`should_switch_frequency_to`'s float→i32 truncation** comparison — a latent
  behaviour bug, needs product input on intent. Out of scope for a refactor.
- **New price providers** (nordpool et al.) — feature work.
- **Adding tests for uncovered logic is in scope only where an existing signature changes**
  (compile-forced). No speculative new coverage.

## Architecture decisions

One decision meets the ADR threshold: **removing the global `CONFIG: OnceCell`**. It is a
crate-wide structural change with a rejected alternative, so it is recorded inline here
rather than as a separate ADR file (single-decision change, no cross-team surface).

- **Choice:** thread configuration as `&Config` (or a narrow extracted struct) through
  `should_switch_frequency_to` and provider construction. Delete the static.
- **Why:** 8 `CONFIG.get().unwrap()` sites, a `_config` binding in `main` that is discarded
  and then re-fetched globally, and two conflicting conventions in adjacent functions
  (`determine_target_mode` takes params, its neighbour reads the global). The static is the
  single reason `should_switch_frequency_to` is untestable.
- **Rejected:** keeping the static and injecting only where tests need it — perpetuates the
  two-convention split and leaves the unwraps live on the production path.
- **Sub-decision:** the price provider needs the fallback price (`prices.default`), not the
  whole config. Pass it as an explicit `ElPrisetJustNuProvider` field, so the provider keeps
  a narrow dependency rather than the full config.

### Source-driven check

Three units touch library APIs and must be verified against official docs before implementing,
not written from memory:

- **U3** — `thiserror` v1/v2 choice, and whether `#[from]` preserves the error chain for
  `reqwest::Error`. Check `docs.rs/thiserror`.
- **U5** — `tokio::time::interval` / `MissedTickBehavior` semantics
  (`docs.rs/tokio`) — only if the drift fix is pulled forward; otherwise not needed.
- **U6** — `tracing` structured-event syntax and using a `tracing_subscriber`
  `FormatEvent`/timer impl instead of the raw fn-pointer cast in `logger.rs:17`.
  Check `docs.rs/tracing-subscriber`.

## Implementation units

Units are ordered to be strictly sequential. U2 is deliberately early: mockito does not
assert on drop, so the 3 async tests currently pass on zero HTTP requests — they are not a
safety net until U2 lands. U6 is last because it rewrites every logging call site.

---

### U1 — Toolchain and formatter/linter gates

**Goal:** make `cargo clippy` runnable and `.rustfmt.toml` honest; pin the toolchain; make
builds reproducible.

**Files:** `.clippy.toml` (rewrite), `.rustfmt.toml` (rewrite), `rust-toolchain.toml`
(create), `.gitignore` (drop the `Cargo.lock` line), `Cargo.lock` (commit — currently
untracked).

**Patterns to follow:** comment style already in `.clippy.toml` (one short comment per key).
Keep `.rustfmt.toml` to keys that are stable *and* non-default.

**Test scenarios:** none — config only. Verification is the tool exit code.

**Verification:**
- `cargo clippy --all-targets` exits 0 (currently hard-errors)
- `cargo fmt -- --check` exits 0
- `rustup toolchain list` shows the pinned version is in use
- `git ls-files \| grep Cargo.lock` returns a line

**Dependencies:** none.

---

### U2 — Make the async tests actually assert

**Goal:** the 4 mockito mocks verify the request actually happened, so U3-U6 have a real
safety net. Delete dead test scaffolding.

**Files:** `tests/unit/bitaxe_tests.rs` (add `.assert()`), `tests/common.rs` (delete),
`tests/tests.rs` (drop the `pub mod common;` declaration).

**Patterns to follow:** mockito's explicit-assert convention — existing
`let mock_restart = ...create();` becomes `mock_restart.assert();` at the end of the test
rather than the current `let _ = mock_restart;`.

**Test scenarios (this unit IS the tests):**
- happy: `test_switch_frequency_success` asserts both PATCH **and** restart were called
- error: `test_switch_frequency_failure` asserts PATCH was called **and** restart was not
- edge: `test_get_running_mode_success` asserts the info request landed
- regression guard: temporarily break the source (e.g. comment out the restart POST) and
  confirm the suite goes **red** — this is the RED step proving the asserts bite

**Verification:** `cargo test` green; plus the deliberate-break check above, reverted.

**Dependencies:** none (independent of U1).

---

### U3 — Consolidate error handling

**Goal:** one error strategy. Remove ~60 lines of hand-written `Display`/`From`/`PartialEq`,
restore lost error sources, delete the dead `PartialEq` impls.

**Files:** `Cargo.toml` (add `thiserror`), `src/bitaxe.rs`, `src/price_providers/price.rs`.

**Patterns to follow:** keep the existing variant *names* — `tests/unit/bitaxe_tests.rs:140`
matches on `BitaxeError::NetworkMessage(msg)` and asserts on its payload string. Variant
names and payload shapes must not change in this unit.

**Test scenarios:**
- existing: all 17 error-path tests still green (they use `.is_err()` / one payload match)
- new: the 2 tests whose whole purpose is the error *message* stay meaningful after the
  `#[from]` conversion — verify the string still contains the cause

**Verification:** `cargo test` green; `cargo clippy --all-targets` green;
`grep -c "impl std::error::Error" src/` → 0.

**Dependencies:** U2 (need the safety net live before touching `src/`).

---

### U4 — Remove the global `CONFIG`

**Goal:** delete `pub static CONFIG: OnceCell<Config>`; no `.get().unwrap()` on config
anywhere; `factory` returns `Result` instead of silently defaulting / panicking.

**Files:** `src/common.rs`, `src/main.rs`, `src/bitaxe.rs`, `src/price_providers/factory.rs`,
`src/price_providers/elpriset_just_nu.rs`, `tests/unit/bitaxe_tests.rs` +
`tests/unit/price_providers/price_tests.rs` (compile-forced signature updates).

**Patterns to follow:** `determine_target_mode` is already the right shape (pure, thresholds
as params) — make every other function match it.

**Test scenarios:**
- new: `should_switch_frequency_to` becomes testable for the first time — no-change case
  (running == target) and change case
- new: factory with an unknown `provider_type` returns `Err`, not a silent default
- new: factory with a missing `[price_provider.elpriset_just_nu]` block returns `Err`,
  not `.expect()` panic
- existing: 29-test suite green, with `CONFIG`-dependent tests converted to injected config

**Verification:** `cargo test` green;
`grep -rn "CONFIG" src/` → 0 hits; `grep -rn "unwrap()" src/` count decreases from 9.

**Dependencies:** U3.

---

### U5 — Decision API, naming, and the 3 live clippy warnings

**Goal:** `should_switch_frequency_to` → `Option<i32>` (kills the `-1` sentinel);
`status() == 200` → `StatusCode::OK`; fix `nonminimal_bool` + `useless_format` + `op_ref`;
rename what misleads (`determine_target_mode`→`target_frequency`, `ten_millis`, the
`switch_frequency_to` param); type the interval so it cannot overflow or panic.

**Files:** `src/bitaxe.rs`, `src/main.rs`, `src/common.rs`,
`tests/unit/bitaxe_tests.rs`.

**Patterns to follow:** U4's injected-config convention; `StatusCode::OK` as already used in
`elpriset_just_nu.rs:41`.

> Note: renaming the three *modes* (`cheap`/`default`/`expensive`) would rename public config
> keys — a breaking change for existing `config.toml` files. Out of scope; README's footnote
> stays. Only function/variable naming is corrected here.

**Test scenarios:**
- new: `None` (not `-1`) when the running frequency already matches the target
- edge: threshold boundaries re-confirmed after rename (existing boundary tests, unchanged
  expectations)
- regression: the 5 `determine_target_mode` tests keep identical assertions under the new name
- error: config with a negative `check_interval` is a **deserialization error**, not a panic

**Verification:** `cargo clippy --all-targets -- -D warnings` green (first time achievable);
`cargo test` green; `grep -rn '\-1' src/` shows no sentinel remaining.

**Dependencies:** U4.

---

### U6 — Structured logging

**Goal:** delete the `logger` string wrapper; call `tracing` macros directly with structured
fields at the ~20 sites; replace the fragile fn-pointer timer cast with a proper
`tracing_subscriber` timer/event impl.

**Files:** `src/logger.rs`, `src/main.rs`, `src/bitaxe.rs`, `src/common.rs`,
`src/price_providers/factory.rs`, `src/price_providers/elpriset_just_nu.rs`.

**Patterns to follow:** none in-repo (this introduces the convention). Keep the existing
`HH:MM:SS` local-time output so log appearance does not regress.

**Test scenarios:** none — logging is not asserted anywhere today and adding a log-capture
layer is not requested.

**Verification:** `cargo test` green; `cargo clippy --all-targets -- -D warnings` green;
`grep -rn "logger::" src/` → 0 hits; `grep -rn "&format!" src/ | grep logger` → 0 hits.

**Dependencies:** U5.

---

### U7 — Dockerfile hardening

**Goal:** dependency layer caching, reproducible build, non-root.

**Files:** `Dockerfile`.

**Patterns to follow:** the existing multi-stage + explicit-comment style (`# Build stage`,
`# Install ca-certificates for HTTPS requests to elprisetjustnu.se`). Keep the existing
`HEALTHCHECK` — it was added deliberately in `a959ed0`.

**Test scenarios:** none — image build only.

**Verification:**
- `docker build -t bitaxe-clocker:refactor .` succeeds
- dependency-caching proof: build once, `touch src/main.rs`, rebuild, and confirm the
  dependency compile layer reports `CACHED`
- `docker run --rm bitaxe-clocker:refactor` still starts and logs (config from the baked
  `config.example.toml`)

**Dependencies:** U1 (`Cargo.lock` must be tracked or `--locked` cannot resolve).

## Review record

**Mode:** CEO Review (user-selected at the plan gate). Approach **B — full 7-unit pass** chosen
over A (gate-first minimal) and C (testability core, logging deferred).

Decisions taken:

| Item | Outcome | Note |
|---|---|---|
| Frequency-comparison risk (`parse_running_mode` `as_f64() as i32` vs integer config) | **Deliberately deferred — user decision** | Promoted to U0 then declined. Not fixed here. Requires real-device verification by the owner. In-repo evidence it is live: `.as_f64()` was chosen over `.as_i64()`, implying the author expected a fractional value, and `tests/fixtures/bitaxe_info.json` is synthetic (never a captured payload). External verification of the Bitaxe firmware field type failed — search gateway timed out 3×. |
| U6 structured logging | **Kept** | Reviewed for cutting as pure churn (20 call sites, no behavioural payoff, only unit touching every file). User elected to keep it. |
| CI | **Excluded — user decision** | Accepted consequence: the clean `clippy -D warnings` state this plan reaches has no automated keeper, which is the same condition that let `.clippy.toml` rot originally. Verification commands below are run by hand. |
| Stale branches | Checked, no collision | `logging` is content-identical to `main`; `better-naming` and `docker` are pre-squash leftovers whose content is already merged. All three also carry a `src/test.rs` that no longer exists on `main`. |

Two implementation decisions resolved during review, so they are not rediscovered mid-unit:

- **U1 clippy config shape:** minimal valid config only. The original file's failure mode was
  invented key names, which produce a confusing build-wide `unknown field` wall rather than a
  pointing error. Do not add a pedantic subset in the same commit.
- **U4 config-passing ceiling:** `determine_target_mode` currently takes 4 params. After U4 no
  function may exceed 4 — prefer a purpose-built narrow struct over widening signatures. The
  price provider receives exactly `prices.default`, never the whole `Config`.
- **U1 commit hygiene:** committing the never-tracked `Cargo.lock` is its own commit, so it does
  not inflate U1's reviewable diff.

### U8 — Relocate unit tests into `#[cfg(test)]` modules

**Goal:** `rust/testing.md` says unit tests belong in `#[cfg(test)]` modules beside the
code; this repo put them under `tests/unit/`, driven by a two-line `tests/tests.rs` shim.
Concrete cost of the current layout: `cargo test --lib` reports **0 tests**, because there
are none in the library.

**Files:** append `#[cfg(test)] mod tests` to `src/bitaxe.rs`, `src/common.rs`,
`src/price_providers/price.rs`, `src/price_providers/factory.rs`; delete `tests/tests.rs`,
`tests/unit/` and its two `mod.rs` files. `tests/fixtures/` stays put — cargo discovers
targets from top-level `tests/*.rs` only, so a fixtures directory alone defines none.

**Patterns to follow:** `rust/testing.md` unit-test pattern (`mod tests { use super::*; ... }`).

**Test scenarios:** none new. **This unit adds zero tests and must not** — it is a pure
relocation, and 40 passing tests before and after is the whole contract.

**Verification:** `cargo test --lib` reports 40 passing (was 0); `cargo test` still 40 total;
`cargo clippy --all-targets -- -D warnings` and `cargo fmt -- --check` clean.

**Dependencies:** U5 and U6 — sequenced last so the move is pure cut-paste, with no signature
or log-call churn left to absorb.

## Review record

**Round 1** — isolated 04-review, 5 reviewer personas. 1 High, 2 Moderate, 8 Low.

The High was a regression introduced by U7 and confirmed by reproduction: rewriting
`Dockerfile` wholesale dropped `COPY . .`, so the builder ran `cargo build` in an empty
`/app` (`error: could not find Cargo.toml`). It survived because the only gate that would
catch it — `docker build` — was the one that could not be run. Restoring that `COPY` then
pulled U1's `rust-toolchain.toml` into the build context, where `channel = "stable"` would
have made rustup try to download a toolchain mid-build; excluded via `.dockerignore`.

Round 1 also overturned one of this plan's own claims. U1 had asserted that
`single_line_if_else_max_width`, `struct_lit_width` and `struct_variant_width` were
nightly-only and inert. The command used to establish that, `rustfmt --config help`, is not
supported by this rustfmt and emitted a single usage line — an empty comparison, not a
result. Two re-tests were inconclusive (wrong file) and confounded (`max_width` alone
already prevented the wrap being tested). The claim was therefore **removed rather than
corrected**: whether those keys act on stable is not established either way.

**Round 2** — incremental, capped. 0 High, 1 Moderate, 5 Low. All verified green:
`cargo test --lib` 41/41, `clippy -D warnings` 0, `cargo fmt --check` 0, and bare
`rustfmt --check src/bitaxe.rs` 0 (the round-1 Moderate is gone).

## Outstanding — before merge

**Run `docker build -t bitaxe-clocker .`.** This is the one gate this branch could not
satisfy: the docker daemon is unavailable in this environment, so U7 shipped only its
low-risk subset (`--locked`, merged apt layer, non-root `USER`) and the F1 fix above is
correct by inspection but **unverified by execution**. A second `docker compose build`
after `touch src/main.rs` would additionally show whether dependency layers are reused —
not attempted, since layer caching was deliberately left out of U7's scope.

Deliberately deferred, by user decision, not by oversight:

| Item | Why deferred |
|---|---|
| Frequency-comparison risk (`parse_running_frequency` truncates `as_f64()` to `i32`, compared against integer config) | Owner elected to verify against hardware rather than have it changed blind. A trial run showed 192.168.8.226 switching 520→530 while .227 matched exactly — consistent with either a legitimate one-time correction or a per-cycle restart loop. Needs one device watched for two cycles. |
| Dockerfile dependency-layer caching | Needs a runnable `docker build` to prove the `CACHED` layer, which was the whole point of the change. |
| Base-image / `rust-toolchain.toml` pairing | Same reason. Currently handled by keeping the toolchain file out of the build context. |
| `check_interval` sleep drift | Changes runtime timing behaviour; this pass promised none. Only the *typing* was fixed. |
| Renaming the `cheap`/`default`/`expensive` modes | Public config keys — renaming breaks existing `config.toml` files. |
| 10 duplicated `Bitaxe { … }` literals across the relocated tests | Noted by review as a P2 follow-up; would have inflated this branch's largest diff for no behavioural gain. |

## Verification strategy

No CI, so these run by hand at each unit boundary, in this order.

Targeted, per unit — the commands in each unit's *Verification* block.

Broader, at the end of every unit:

1. `NODE_ENV=test cargo test` → expect 29 passing (U2 may add; U4/U5 change counts)
2. `cargo clippy --all-targets -- -D warnings` → 0 (only meaningful from U1 onward)
3. `cargo fmt -- --check` → 0
4. `cargo build --release` → 0

Behaviour-preservation check (the important one, since this refactor promises no behaviour
change): build before (`git stash` at U1 start) and after, then diff the observable surface —
same `config.toml`, same request/response sequence against a stubbed Bitaxe, and confirm the
emitted `PATCH`/`POST restart` pattern is unchanged.

Manual smoke test if a real device is reachable: run against a configured host for two
`check_interval` cycles and confirm the second cycle logs "no change" rather than restarting.

Final: `git diff --stat main..HEAD` reviewed per file, then `04-review`.
