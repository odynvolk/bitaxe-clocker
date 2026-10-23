---
isolation: isolated
---

# Review — `refactor/code-quality` vs `main`

`Rules loaded:` language=**rust** (via changed `src/**/*.rs` + `Cargo.toml`; no `{repo-root}/rules/` directory, so package defaults apply),
common=`rules/common/code-review.md`, `rules/common/code-smells.md`,
lang=`rules/rust/coding-style.md`, `rules/rust/patterns.md`, `rules/rust/security.md`, `rules/rust/testing.md`, `rules/rust/hooks.md`,
`rules/rust/review-checklist.md` — **missing (fell back to common)**,
web=N/A (no frontend/browser changes).

Diff scope: `git diff main...HEAD` — 23 files, +3198 / −797 (of which `Cargo.lock` is +1958 of generated content).
Reviewer personas from `review_router`: correctness, testing, maintainability, integration, thoroughness.
Spec source: `docs/plans/2026-10-23-code-quality-refactor-plan.md` (U1–U8).
Solution search: `docs/solutions/` does not exist in-repo and `~/.pi/agent/docs/solutions/` does not exist — 0 candidates.

Verification run by this reviewer (read-only, no source edits):
`NODE_ENV=test cargo test --lib` → 40 passed / 0 failed · `cargo clippy --all-targets -- -D warnings` → exit 0 ·
`cargo fmt -- --check` → exit 0 · `git ls-files | grep Cargo.lock` → present · `rustc --version` → **1.95.0**.

---

## Findings

### F1 — HIGH — `docker build` cannot succeed: the builder stage no longer copies the crate

- **severity:** high
- **summary:** U7 deleted the only `COPY` into the builder stage without replacing it, so `cargo build --release --locked` runs against an empty `/app` and aborts.
- **evidence:** `Dockerfile` — the only `COPY`/`ADD` instructions in the whole file are at lines 43 and 45, and both are in the **runtime** stage (`COPY --from=builder /app/target/release/bitaxe-clocker` and `COPY … config.example.toml`). The builder stage is `FROM rust:1.85-slim` → `WORKDIR /app` → `RUN apt-get …` (line 7) → `RUN cargo build --release --locked` (line 21). Nothing between them places `Cargo.toml`, `Cargo.lock` or `src/` in the image.
  Reproduced with the exact command against an empty directory:
  ```
  $ cargo build --release --locked
  error: could not find `Cargo.toml` in `…` or any parent directory
  ```
  `main` had `COPY . .` immediately before `RUN cargo build --release`; the diff removes those two lines. Commit `4cbd745` states "The dependency-layer-cache restructure … deliberately left out" — but the *existing* copy was removed, so the regression is not "caching not added", it is "build removed".
  The U7 verification gate (`docker build -t bitaxe-clocker:refactor .` succeeds, plus the `touch src/main.rs` → `CACHED` proof) was never run — the current handoff records `NOT verified: docker build (daemon off)`.
- **impact:** every documented run path breaks — `README` "build and run with plain Docker", `docker compose up -d` (`docker-compose.yml:2` → `build: .`), and therefore the `HEALTHCHECK`/`restart: unless-stopped` path added in `a959ed0`.
- **recommended action:** restore a working builder before merge. Minimum correct shape for both goals:
  `COPY Cargo.toml Cargo.lock ./` + `COPY src ./src` before `RUN cargo build --release --locked` (or a `mkdir src && echo "fn main(){}" > src/main.rs` dep-only layer, then copy real `src` and `touch src/main.rs`). Then run the two U7 verification commands for real; if docker genuinely cannot be run, restore plain `COPY . .` + `cargo build --release --locked` and reopen U7 rather than ship a broken build.
- **related plan unit:** U7 — Dockerfile hardening
- **reviewer:** integration-reviewer / correctness-reviewer
- **autofixable:** no (needs a build-environment decision + a real `docker build` to verify)

### F2 — MODERATE — `channel = "stable"` is not a pin, and is not in sync with the image

- **severity:** moderate
- **summary:** The plan's U1 goal was to *pin* the toolchain; the file pins a floating channel, so the dev/Docker drift that motivated the unit is still present and now larger.
- **evidence:** `rust-toolchain.toml:5` → `channel = "stable"`. Measured on this branch: `rustc --version` → `1.85`-claimed vs **`rustc 1.95.0`** locally. `Dockerfile:2` is a pinned `rust:1.85-slim`. `rust-toolchain.toml:4` claims "The Docker base image is kept in sync with this channel (see Dockerfile)" — `stable` ≠ `1.85`, and no instruction copies the file into the builder (F1), so the image never reads it. `README.md:13-14` repeats the claim: "The required toolchain is pinned in `rust-toolchain.toml` … no need to install a specific version by hand." The deleted `.tool-versions` at least named a concrete `1.90.0`.
  Note the author's own commit message for `4cbd745` flags the pairing as risky: "the second one can make a build depend on a rustup download" — the concern is recorded, but the shipped comment asserts the pairing is done.
- **recommended action:** pick one source of truth and make the comment true. Either `channel = "1.85.0"` in `rust-toolchain.toml` (then the README's "pinned" claim holds and the image matches), or keep `stable` and change the base to `rust:1-slim`, and correct the comment to say the channel floats. If the toolchain file is meant to apply inside the image, it must be `COPY`ed into `/app` — and then re-verify the build does not pull a second toolchain via rustup.
- **related plan unit:** U1 — Toolchain and formatter/linter gates
- **reviewer:** integration-reviewer
- **autofixable:** yes (single-line change, but the choice needs the author's decision)

### F3 — MODERATE — removing `edition = "2021"` breaks bare `rustfmt` and contradicts the file's own rule

- **severity:** moderate
- **summary:** `.rustfmt.toml:3` states only keys that are "stable **and** non-default" are listed; `edition` is stable and non-default (defaults to `2015`), yet it was dropped — so direct `rustfmt` invocation now fails to parse the crate.
- **evidence:** Reproduced in this repo at HEAD:
  ```
  $ rustfmt --check src/bitaxe.rs
  error[E0670]: `async fn` is not permitted in Rust 2015
    --> src/bitaxe.rs:37:5      (exit status non-zero)
  $ rustfmt --edition 2021 --check src/bitaxe.rs
  (no output, exit 0)
  ```
  `edition = "2021"` was present on `main` (`git diff main...HEAD -- .rustfmt.toml`), so this is a regression introduced by U1. `cargo fmt` is unaffected — cargo passes the edition from `Cargo.toml` — which is why the U1 gate (`cargo fmt -- --check` → 0) could not catch it.
- **impact:** any tooling that shells out to `rustfmt` rather than `cargo fmt` (editor formatters not driven by rust-analyzer, ad-hoc `rustfmt` runs, hand-written pre-commit hooks — and this repo has no CI, so ad-hoc commands are the workflow the README is teaching) now fails with a confusing 2015-edition parse error instead of a formatting diff.
- **recommended action:** restore `edition = "2021"` with a one-line comment ("not the default; needed for non-`cargo fmt` invocations"). This also makes the file consistent with its own stated inclusion rule.
- **related plan unit:** U1
- **reviewer:** integration-reviewer / thoroughness-reviewer
- **autofixable:** yes

### F4 — LOW — the `.rustfmt.toml` header comment's own claims are wrong

- **severity:** low
- **summary:** The comment explaining *why* keys were dropped misclassifies them, so the lesson it teaches is the opposite of the real one.
- **evidence:** `.rustfmt.toml:6-8` — "of 12 keys, 9 were rustfmt defaults and 3 (`single_line_if_else_max_width`, `struct_lit_width`, `struct_variant_width`) were nightly-only and inert."
  Checked key-by-key against `rustfmt --help=config` (rustfmt 1.9.0-stable): exactly **7** of the 12 equal the stable defaults — `hard_tabs=false`, `tab_spaces=4`, `reorder_imports=true`, `merge_derives=true`, `remove_nested_parens=true`, `match_arm_leading_pipes=Never`, `use_try_shorthand=false`. Two are non-default and were effective: `max_width=120` (default 100) and `edition=2021` (default 2015 — see F3). And `struct_lit_width` / `struct_variant_width` are **not** nightly-only: they appear in the stable option list, and `rustfmt --edition 2021 --config struct_lit_width=1 --check` demonstrably changes output on stable. (`struct_lit_width = 18` was inert for a different reason — 18 *is* the default. `struct_variant_width` is unobservable in this crate: there are no struct enum variants, so 16 vs the 35 default has no effect.)
- **impact:** none on formatting (`cargo fmt -- --check` is clean). The cost is that the next maintainer trusts an incorrect rationale.
- **recommended action:** restate accurately — e.g. "7 of 12 equalled the stable default and did nothing; `struct_lit_width` equalled its default; `single_line_if_else_max_width` is nightly-only; `max_width` and `edition` were the only two effective keys, and both are kept."
- **related plan unit:** U1
- **reviewer:** thoroughness-reviewer
- **autofixable:** yes

### F5 — LOW — U6 left one log call site unstructured, at the site with the most to say

- **severity:** low
- **summary:** `tracing::error!("Error getting price using default")` carries no fields — the only such call remaining in `src/` — and its neighbouring doc comment names the wrong trigger.
- **evidence:** `src/price_providers/elpriset_just_nu.rs:47`, inside the `else` branch of `if response.status() == StatusCode::OK`. The three things an operator needs — the status actually received, the `url`, and the value taken as the fallback — are all in scope at that line and none is recorded. Every other U6 call site was converted to structured fields (`url = %url`, `frequency`, `host = %bitaxe.host`, `sek_per_kwh`, `minutes`, `config = ?config`), so this is a gap against the unit's own convention rather than a stylistic preference.
  Separately, `elpriset_just_nu.rs:17-18` documents `fallback_price` as used "when the price API is unreachable". The actual trigger is a **non-2xx response**; a genuinely unreachable host fails at line 42 and propagates `PriceError::Network` via `?`, never reaching the fallback.
- **recommended action:** `tracing::error!(url = %url, status = %response.status(), fallback_price = self.fallback_price, "Price request failed, using configured default");` and reword the field doc to "when the price API returns a non-OK status".
- **related plan unit:** U6 — Structured logging
- **reviewer:** maintainability-reviewer
- **autofixable:** yes

### F6 — LOW — the "available providers" list is duplicated between code and a hand-maintained error string

- **severity:** low
- **summary:** `ConfigError::UnknownProvider`'s message hardcodes one provider name in `common.rs`, while the set of accepted names lives in the `factory.rs` match; nothing keeps them together.
- **evidence:** `src/common.rs:14` — `#[error("unknown price provider '{0}'; expected one of: elpriset_just_nu")]`. The accepted set is `src/price_providers/factory.rs:19-32`. `README.md:260` turns the coupling into a manual step: "A new provider needs a matching arm in `ConfigError::UnknownProvider`'s message so the error names the providers that are actually available." With no CI (plan: CI excluded by user decision) nothing catches the miss, so the second provider ships an error message that tells the operator the only valid value is the first one — i.e. the new fail-fast path F1/F2-style ships with a wrong diagnostic.
  Possible **Feature Envy** / **Primitive Obsession** (Fowler baseline, P2): the provider registry is a stringly-typed match, and knowledge about it lives in a sibling module.
- **recommended action:** give the factory the single source of truth — e.g. `ConfigError::UnknownProvider { found: String, expected: &'static [&'str] }` built from one `SUPPORTED_PROVIDERS` const that the match also reads, so the message cannot go stale and `common.rs` stops naming providers at all.
- **related plan unit:** U4 — Remove the global `CONFIG`
- **reviewer:** maintainability-reviewer
- **autofixable:** no (API shape decision)

### F7 — LOW — two new tests assert that nonsense device input is correct

- **severity:** low
- **summary:** `test_parse_running_frequency_float_frequency` and `…_negative_frequency` green-light the truncation and a negative frequency with no marker that either is unverified.
- **evidence:** `src/bitaxe.rs:430` asserts `{"frequency": 550.7}` → `Ok(550)`; `src/bitaxe.rs:464` asserts `{"frequency": -100}` → `Ok(-100)`. Both pass today (`src/bitaxe.rs:49` is `as_f64()… as i32`, unchanged from `main`), so neither is a regression. The problem is signalling: the float-truncation behaviour is exactly the item the plan **deferred by user decision** — plan Review record: "`parse_running_mode` `as_f64() as i32` vs integer config … Requires real-device verification by the owner", plus "`.as_f64()` was chosen over `.as_i64()`, implying the author expected a fractional value" and "`tests/fixtures/bitaxe_info.json` is synthetic (never a captured payload)". The current handoff adds field evidence: host `.226` was observed "switching 520->530 while `.227` matched exactly".
  After this diff, the only signal inside `src/bitaxe.rs` is a passing test that reads "550.7 → 550 is the expected result" — which converts an open risk into an apparent invariant, and makes the future fix look like a test regression.
- **impact:** behavioural risk stays exactly where the plan put it (deferred). This finding asks only that the deferral be visible at the code, not that the truncation be changed here.
- **recommended action:** do **not** change behaviour (out of scope by user decision). Mark both as characterization instead: rename to e.g. `characterization_float_frequency_truncates_toward_zero`, and add one short comment naming the deferred item and the fixture-is-synthetic caveat, so the next reader searches for the deferral rather than trusting the green tick. Same for `-100`: state that no range validation exists on the parsed frequency.
- **related plan unit:** U8 — Relocate unit tests into `#[cfg(test)]` modules
- **reviewer:** testing-reviewer / correctness-reviewer
- **autofixable:** no (naming/comment intent)

### F8 — LOW — the same fixture is read two ways, and `src/` tests reach into `tests/`

- **severity:** low
- **summary:** One fixture is loaded via a CWD-relative `fs::read_to_string` in one test and `include_str!` in another; after U8, `tests/` holds no test targets at all.
- **evidence:** `src/bitaxe.rs:169` — `std::fs::read_to_string("tests/fixtures/bitaxe_info.json").unwrap()` (resolves against the test process CWD). `src/bitaxe.rs:375` — `include_str!("../tests/fixtures/bitaxe_info.json")` (resolved at compile time) — for the same file. Both pass today because cargo sets the test binary's CWD to the package root. `find tests -type f` → only `tests/fixtures/bitaxe_info.json` and `tests/fixtures/price.json`; `tests/tests.rs`, `tests/common.rs` and both `mod.rs` files were deleted, so the crate now has 40 unit tests and **zero** integration test targets, while unit tests cross the src↔tests boundary by relative path.
  Possible **Duplicated Code** (Fowler baseline, P2): two mechanisms for one fixture.
- **recommended action:** use `include_str!` in both places (drops the CWD dependency and the `.unwrap()` on I/O) or move the two fixtures next to the modules that consume them so unit tests stop reaching into a directory that no longer contains tests.
- **related plan unit:** U8
- **reviewer:** testing-reviewer
- **autofixable:** yes

### F9 — LOW — 10 copies of the same `Bitaxe` literal across the relocated test module

- **severity:** low
- **summary:** The test bodies were moved verbatim and the fixture construction they already duplicated got moved with them.
- **evidence:** `src/bitaxe.rs` — `expensive: 50, default: 550, cheap: 590` appears 10× across the 22 tests in `mod tests` / `mod parse_running_frequency_tests`; only `host` varies (`mock_server.url().replace("http://", "")` ×3, one literal IP, remainder a hard-coded `"192.168.8.227"`). A `bitaxe_fixture(host: &str) -> Bitaxe` next to the existing `prices()` (`src/bitaxe.rs:219`) and `device_running_at()` (`src/bitaxe.rs:229`) helpers covers all ten. The 9 `parse_running_frequency_*` cases (`src/bitaxe.rs:358-471`) differ only in their input JSON and share identical assert shape.
- **note:** the plan's U8 contract was "This unit adds zero tests and must not — 40 passing tests before and after is the whole contract", so the duplication is inherited rather than introduced. Reported as P2 only because the helpers the file already has make the collapse mechanical.
- **recommended action:** optional, and cleaner as a follow-up commit than inside the relocation — extract the one `Bitaxe` helper; leave the 9 parse cases as-is unless a `rstest` `#[case]` table is welcome (adds a dev-dependency, so judge against the plan's dependency-minimal stance).
- **related plan unit:** U8
- **reviewer:** maintainability-reviewer / thoroughness-reviewer
- **autofixable:** yes

### F10 — LOW — README's hand-run gate documents a JS-only env var

- **severity:** low
- **summary:** `NODE_ENV=test cargo test` is presented as the repo's test command; no tool in this dependency tree reads `NODE_ENV`.
- **evidence:** `README.md:24`, inside the new "There is no CI in this repository, so these checks are run by hand" block. The tree is Rust-only (`Cargo.toml`: chrono, reqwest, serde, serde_json, thiserror, toml, tokio, tracing, tracing-subscriber; dev: mockito) — none of these, nor cargo/rustc, read `NODE_ENV`.
- **impact:** with no CI, this README block *is* the quality gate, so a bogus prefix on it is the kind of thing that gets copied into the next command and then defended as convention.
- **recommended action:** drop the prefix → `cargo test`. (The global rule this came from scopes `NODE_ENV=test` to JavaScript/TypeScript projects; it does not apply to this repo.)
- **related plan unit:** U1 (README "Development" section added alongside the toolchain work)
- **reviewer:** maintainability-reviewer
- **autofixable:** yes

### F11 — LOW — two shipped startup-behaviour changes have no upgrade note

- **severity:** low
- **summary:** The plan frames the whole branch as "a no-behaviour-change refactor", but two previously-tolerated misconfigurations now abort startup, and no user-facing note says so.
- **evidence:** Plan *Scope boundaries*: "**Behaviour changes.** This is a no-behaviour-change refactor." Against that, on this branch an unrecognised `provider_type` used to log `"Unknown provider type '{}', using elpriset_just_nu"` and continue (`git show main:src/price_providers/factory.rs`), and a missing `[price_provider.elpriset_just_nu]` section used to panic via `.expect()`; both now return `ConfigError` from `PriceProviderFactory::create_provider` (`src/price_providers/factory.rs:15-33`) and exit `main` at `src/main.rs:35`. Intended — the plan's U4 and the new tests at `factory.rs:60`/`:68` both call it out — so this is a documentation gap, not a wrong implementation.
  Nothing records it for whoever upgrades: there is no `CHANGELOG`, and the README Config Keys section gained only the `check_interval` `i32`→`u64` row.
- **recommended action:** one line under README Usage/Docker: "A `provider_type` that is not `elpriset_just_nu`, or a missing `[price_provider.elpriset_just_nu]` section, now fails at startup instead of falling back." Also soften the plan's blanket "no behaviour change" claim to "no behaviour change except the three startup diagnostics listed in U4/U7".
- **related plan unit:** U4
- **reviewer:** correctness-reviewer / spec axis
- **autofixable:** yes

---

## Checked and deliberately not reported

Recorded so they are not re-raised:

- **`PriceError::Network` / `BitaxeError::Network` source chain** — genuinely preserved: `network_failure_keeps_the_transport_error_as_source` (`src/bitaxe.rs:192`) passes and asserts a real `err.source()`; the assertion is not satisfiable by the other two variants (both are source-less), so it bites.
- **U2 mockito assertions** — every mock created is asserted; `test_switch_frequency_failure` uses `.expect_at_most(0)` + `.assert()` for the "must not restart" case, and the suite is green.
- **`fallback_price` semantics vs `main`** — identical. `main` passed `config.prices.default`; `main`'s old `CONFIG.get().ok_or(PriceError::InvalidPrice)?.prices.default` always resolved because `load_config()` set the cell.
- **`check_interval` `i32`→`u64`** — existing valid configs still parse (`u64` is a superset for positive values); README row updated; the overflow path is now `checked_mul` + `ConfigError::IntervalTooLarge` with a test at `i64::MAX` (`src/common.rs:140`).
- **`tracing_subscriber` `chrono` feature** — valid, not a typo: it is the implicit feature of the optional `[dependencies.chrono]` (`tracing-subscriber-0.3.23/Cargo.toml:231`, `optional = true`), and `fmt::time::ChronoLocal` is gated on it.
- **`-1` for a configured frequency** — the old `-1` sentinel *could* swallow a real target frequency if an operator wrote `cheap = -1`; `Option<i32>` removes that class. Not flagged as a regression.
- **Ctrl-C does not interrupt an in-flight `tokio_time::sleep`** — the shutdown flag is only polled at the top of each iteration (`src/main.rs:38`). Unchanged behaviour from `main`, and the plan puts `check_interval` timing semantics out of scope. Not a diff finding.
- **Full-config logging** (`src/common.rs:82`, `src/main.rs:18`) — dumps device hosts. Same as `main`'s `Config: {:?}`; the config struct holds no credentials today. Speculative, so not reported.
- **`cargo audit` / `cargo deny` not wired up** — `rules/rust/security.md` suggests them, but with CI user-excluded by plan decision, requesting them here would be cargo-cult rather than a defect.
- **Issues rustfmt/clippy/compiler already catch** — none reported; both gates pass at HEAD.
- **`.clippy.toml` deleted rather than rewritten** (plan U1 said "rewrite") — accepted: an empty config is the minimal valid config, which is what the plan's own U1 decision prescribes.

---

## Review summary

- **rules applied** — language=`rust` (detected from changed `src/**/*.rs` + `Cargo.toml`; no `{repo-root}/rules/language-detection.md` and no project `rules/` directory, so no project-level override).
  common: `rules/common/code-review.md`, `rules/common/code-smells.md`.
  rust: `coding-style.md`, `patterns.md`, `security.md`, `testing.md`, `hooks.md`.
  `rules/rust/review-checklist.md`: **missing (fell back to common)**.
  `rules/web/`: N/A.
  Project rules from `AGENTS.md` honoured: no `.env` access, no `.data/`/`public/` writes, comments kept short.
- **findings by severity:** high **1** · moderate **2** · low **8**
- **verification status:** all 11 findings confirmed against code by direct read or by reproducing the command; **0 pushed back**, **0 deferred** by this reviewer. Two items are *deliberately not asked for*: the `parse_running_frequency` truncation behaviour itself (plan/user deferred pending hardware) and the CI gate (plan/user excluded).
- **blocking:** **F1 only.** Everything else is non-blocking.
- **author-side next step (not performed here — this session is terminal):** fix F1 and re-run the two U7 docker commands; F2/F3 are one-line config corrections worth folding into the same pass. F4–F11 may be recorded in the handoff as P2 follow-ups without re-entering the review loop.
- **plan status:** `docs/plans/2026-10-23-code-quality-refactor-plan.md` still reads `> Status: executing`; flip to `done` and move to `docs/plans/archive/` only once F1 is fixed and the U7 gate has actually been run.
