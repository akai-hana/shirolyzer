# shirolyzer Improvement Review

Review date: 2026-11-17

This is a static and test-backed review of the repository at
`/home/akai/git/shirolyzer`. It is intentionally broader than a style pass:
the review covers scoring semantics, search behavior, configuration and data
validation, corpus handling, CLI behavior, reproducibility, testing, tooling,
documentation, security, licensing, and repository hygiene.

No source files were changed. This document is the only intended repository
change.

## Executive summary

shirolyzer has a strong core: the Rust modules are cleanly separated, the
scoring/search pipeline is deterministic when seeded, the test suite exercises
the shipped baseline and several end-to-end commands, and the codebase contains
no `unsafe` blocks. With a clean build directory, all existing tests and lint
checks pass.

The main risks are not general code quality. They are:

1. **Latent scoring inconsistency around spaces.** `score_layout`, the
   generator's `Evaluator`, and `score_breakdown` do not agree about how space
   trigrams are represented. The shipped corpora happen to contain no spaces,
   which hides the problem.
2. **Documented balance behavior does not match the implementation.** The
   balance guard is always added, even when `finger_balance` is absent from
   `priority_list`.
3. **Input validation is incomplete.** A layout with repeated or missing
   letters can produce a misleading report, and non-ASCII punctuation can reach
   byte-indexed slicing and panic. A negative, infinite, or `NaN` time budget
   can panic in the generator.
4. **Corpus loading is not hardened against resource exhaustion.** XZ, ZIP, and
   JSON inputs are read and decompressed without size limits.
5. **Corpus weighting is raw-count based.** Multiple corpora are scaled and
   summed without normalizing to a common distribution, so configured weights
   do not necessarily mean what users expect.
6. **Reproducibility metadata is incomplete.** Generated READMEs omit the
   generated seed from the "Reproduce" command, and commands are recorded
   without shell quoting.

The existing tests are green, but they do not cover several of the failure
modes above. A clean target directory is also important: the checked-in
workspace can contain a stale `target/` copied from another working tree, which
can make tests report false failures.

## Verification baseline

Commands run:

```sh
cargo fmt --check
CARGO_TARGET_DIR=/tmp/opencode/shirolyzer-target cargo clippy --all-targets -- -D warnings
CARGO_TARGET_DIR=/tmp/opencode/shirolyzer-target cargo test --release
```

Results with the isolated target directory:

- `cargo fmt --check`: pass.
- `cargo clippy --all-targets -- -D warnings`: pass.
- `cargo test --release`: pass.
  - 126 library tests.
  - 1 baseline test.
  - 15 command tests, plus 1 ignored `generate_smoke`.
  - 6 parity tests.
  - 0 doctests.

Important caveat: an earlier run using the repository's default `target/`
produced false failures:

- `baseline` expected a shiro score of `3.960357335158009` but saw
  `5.386923720150203`.
- `parity` panicked with values `0,0,0`.
- `commands` failed because the test binary embedded
  `CARGO_BIN_EXE_shirolyzer=/home/akai/git/layout-opencode-investigation/target/release/shirolyzer`.

The test binary contained stale artifacts copied from another working tree, and
Cargo reused them because the stale target directory was present. This is an
environment/build-cache issue, not evidence of a source regression. Clean or
isolate the target directory before trusting local test results.

Git history and status were not used for this review: the repository reports
`fatal: detected dubious ownership in repository at
'/home/akai/git/shirolyzer'` when run as the current user. With an explicit safe
directory override, the working tree appeared clean, but the review does not
depend on git metadata.

## What already works well

- **Clear module boundaries.** Configuration, geometry, model tables, scoring,
  search, corpus loading, analysis, generation, auditing, and rendering are
  separated into focused modules rather than one monolithic command.
- **Strong core regression coverage.** The baseline, parity, and command tests
  cover the shipped configuration, stored scores, and major subcommands.
- **Deterministic search structure.** Seeded RNG use, explicit seed reporting,
  and reference metadata make runs mostly reproducible when the complete
  command line is preserved.
- **Auditing and provenance.** The audit layer and corpus provenance metadata
  are unusually thorough for a project of this size.
- **Useful configuration validation.** Missing required sections, invalid
  finger tokens, duplicate punctuation, non-finite percentages, and several
  invalid numeric ranges are already rejected with contextual errors.
- **Performance awareness.** Precomputed tables, Rayon parallelism, and
  incremental local-search scoring show that performance was considered.
- **No `unsafe` code.** The code avoids unsafe blocks and passes Clippy with
  warnings denied.

## Findings

### High priority

#### H1. Space handling is inconsistent across the three scoring paths

**Evidence**

- `src/evaluate.rs:42` remaps `Corpus::SPACE_INDEX` (`26`) to the scorer's
  `SPACE_ID` (`30`) for `score_layout`.
- `score_layout` includes any remapped trigram whose space sentinel is present
  (`src/evaluate.rs:46-65`).
- `Evaluator::new` filters corpus trigrams to IDs below `26`:
  `src/evaluate.rs:126-135`.
- `src/analyze.rs:50-58` builds a key map from layout characters to physical key
  indices. The map has 27 slots: 26 letters plus `SPACE_INDEX`.
- `score_breakdown` iterates corpus trigrams through that map and checks
  `keys.contains(&SPACE_ID)` at `src/analyze.rs:351`, then calls
  `add_space_terms` at `:352`.
- `add_space_terms` is defined at `src/analyze.rs:296-318`.
- The physical key indices produced by `keymap` are in `0..30`; `SPACE_ID` is
  `30`. A layout's space character maps to a physical key index, not to `30`.
  Therefore `keys.contains(&SPACE_ID)` is effectively unreachable for normal
  layouts, and the space branch in the breakdown is dead.
- `src/analyze.rs:331` obtains `model.tables_unrestricted(geom)` for the
  breakdown, while the primary score uses `tables.tri`; those tables can differ
  when conditions or explicit unreachable-finger allowances are configured.
- The breakdown regression test at `src/analyze.rs:1873` only uses the shipped
  no-space corpus.

**Impact**

The default packaged corpora contain no space trigrams, so the shipped
baseline does not expose the divergence. External corpora, raw corpora that
contain spaces, or future space-aware data can make:

- `analyze`'s primary score differ from the generator objective;
- `score_breakdown` fail to reconstruct the primary score;
- `score_breakdown` silently omit space costs; and
- `README.md:660`'s claim that analyzer scores reflect the generator's
  constraints become inaccurate.

Even if the branch were reachable, `add_space_terms` receives physical-key
fingers from `tables.tri_f` rather than the dedicated `SPACE_FINGER`, so the
result would still need explicit review.

**Recommendation**

- Define one canonical item representation for space in all three paths. Either
  include `SPACE_ID` consistently in `Evaluator`, `score_layout`, and
  `score_breakdown`, or explicitly reject corpora containing spaces with a
  clear error until space-aware generation is supported.
- Remove the sentinel check `keys.contains(&SPACE_ID)`; use the remapped item
  IDs or the same `remap_for_layout` path used by `score_layout`.
- Make `score_breakdown` use exactly the same scoring tables and model
  configuration as the primary score, or document and test the difference.
- Add tests with a synthetic corpus containing space bigrams/trigrams, and
  assert that `score_layout`, the generator's `Evaluator`, and
  `score_breakdown` agree for both space-bearing and space-free corpora.

#### H2. Layout validation is too weak, and analysis can panic on non-ASCII input

**Evidence**

- `src/analyze.rs:1245` checks only `layout.chars().count() != 30`.
- `keymap` silently ignores any character that is not a lowercase ASCII letter
  or a space (`src/analyze.rs:50-58`). Repeated letters, missing letters, and
  unsupported punctuation therefore do not produce an error.
- Grid rendering collects characters safely at `src/analyze.rs:1370-1372`, but
  then slices the resulting `String` by byte offsets:
  `&row[..5]` and `&row[5..]` at `src/analyze.rs:1373`.
- If a row contains a multi-byte character at the split point, those byte
  slices are not character boundaries and panic.
- `src/config.rs:1746-1780` permits non-ASCII punctuation, so this path is
  reachable with a valid-looking config and a 30-character layout.

**Impact**

- A typo such as a duplicated letter can be silently ignored and produce a
  report for a different effective layout.
- A user-supplied Unicode punctuation character can crash `analyze` instead of
  returning a useful validation error.
- Other commands that accept layouts may inherit inconsistent validation
  behavior, making failures hard to diagnose.

**Recommendation**

- Validate layouts explicitly: exactly 26 distinct ASCII letters, exactly four
  configured punctuation characters (or whatever punctuation policy the
  project chooses), no whitespace in the letter area, and no unsupported
  characters.
- Reject duplicate and missing letters with an error that identifies the
  offending character or position.
- Avoid byte-indexed string slicing for layout grids. Build rows from
  `Vec<char>` or split by character count.
- Add tests for duplicate letters, missing letters, uppercase letters,
  punctuation-only layouts, and multi-byte punctuation.

#### H3. The balance guard contradicts the documented priority semantics

**Evidence**

- `README.md:517-519` says the balance penalty is off whenever
  `finger_balance` is absent from `priority_list`.
- `src/evaluate.rs:17-18` defines `BALANCE_FLOOR_SHARE = 0.5` and
  `BALANCE_GUARD_WEIGHT = 10.0`.
- `penalty_from_usage` unconditionally adds
  `balance_guard_penalty(usage, &self.flat_share)` at
  `src/evaluate.rs:323-327`.
- `priority_multipliers` can set the normal balance weight to zero, but that
  does not remove the fixed guard.
- The shipped config and the `tests/fixtures/config.reference.yaml` fixture are
  not aligned on this behavior: the fixture comments out `finger_balance` while
  the implementation still applies the guard.

**Impact**

A user who removes `finger_balance` from `priority_list` still gets a hidden
balance objective. This changes rankings and generator behavior in a way that
the documentation says should not happen, and it makes "disable balance"
impossible without a code change or an explicit guard flag.

**Recommendation**

- Decide whether the guard is an invariant or a configurable objective.
- If it is meant to be off when `finger_balance` is absent, gate
  `balance_guard_penalty` on the configured weight or an explicit enabled flag.
- If the guard is intentionally mandatory, document it as such, rename the
  setting or remove the misleading README claim, and consider exposing a
  separate `balance_guard` option.
- Add a regression test with `finger_balance` present and absent from
  `priority_list`, checking both the computed penalty and a known ranking.

#### H4. Corpus decompression and JSON loading have no resource limits

**Evidence**

- XZ input is read fully into memory, then decompressed into another unbounded
  `Vec<u8>` at `src/corpus.rs:319-327`.
- ZIP input selects the first member whose name ends in `.txt`, then reads that
  member fully into memory at `src/corpus.rs:329-343`.
- JSON loading uses `std::fs::read_to_string` without a size cap at
  `src/corpus.rs:251-254`.
- Source-directory discovery only accepts a single candidate `.txt`, `.xz`, or
  `.zip` file; otherwise it silently returns `None` (`src/corpus.rs:278-299`).

**Impact**

A malformed, accidentally huge, or malicious corpus can exhaust memory. ZIP and
XZ are especially risky because a small compressed file can expand to a very
large output. The "first `.txt` member" policy also makes archive contents
ambiguous and can select the wrong file without warning. Invalid UTF-8 is
replaced lossily at `src/corpus.rs:327` and `:343`, which can silently change
the corpus.

**Recommendation**

- Stream decompression where possible, and enforce explicit compressed and
  decompressed size limits.
- Reject archives with multiple candidate text members unless the user selects
  one explicitly.
- Validate decoded text as UTF-8 and report invalid input instead of silently
  replacing bytes.
- Add a maximum JSON file size and clear errors for oversized or malformed
  inputs.
- Document the accepted archive formats, member-selection rule, and limits.
- Add tests for oversized input, multiple archive members, malformed UTF-8,
  and a decompression bomb guard.

#### H5. A negative, infinite, or `NaN` time budget can panic

**Evidence**

- `parse_budget` at `src/generate.rs:34-51` accepts any string that parses as
  `f64`; it does not reject negative, non-finite, or excessively large values.
- The parsed budget is converted with
  `std::time::Duration::from_secs_f64(budget)` at `src/generate.rs:1098-1100`.
- `Duration::from_secs_f64` panics for negative values, `NaN`, infinity, or
  values that overflow the duration representation.

**Impact**

An input such as `--time-budget -1`, `--time-budget nan`, or
`--time-budget inf` can crash the process after configuration and search setup
rather than producing a validation error. Other numeric CLI flags are also
sparsely validated, so related bad inputs may lead to confusing failures or
nonsensical search behavior.

**Recommendation**

- Require `budget.is_finite() && budget >= 0.0`, and impose a practical upper
  bound.
- Treat zero as "no work" or reject it explicitly; do not rely on downstream
  behavior.
- Use `Duration::try_from_secs_f64` or a checked conversion, with a clear error.
- Validate related numeric flags (`--primary-sacrifice`, tolerances, iteration
  counts, candidate counts, and similar values) for finiteness and sensible
  ranges.
- Add CLI tests for negative, zero, `NaN`, `inf`, and very large values.

#### H6. Corpus scaling is raw-count based and counts can be invalid

**Evidence**

- `Corpus::scaled` multiplies raw letter, bigram, and trigram counts by a factor
  (`src/corpus.rs:83-95`).
- `Corpus::merge` sums raw counts (`src/corpus.rs:97-126`).
- `Corpus::new` computes totals from all map keys before filtering unsupported
  keys into `bi_ids`/`tri_ids` (`src/corpus.rs:44-68`).
- `from_raw_maps` accepts any `f64`; negative values and `NaN` are not rejected
  (`src/corpus.rs:182-208`).
- `load_parsed` reads the three JSON files and constructs a corpus without
  verifying that letter, bigram, and trigram totals are mutually consistent
  (`src/corpus.rs:147-161`).
- Raw text counting flushes the current run on every non-ASCII-lowercase
  character (`src/corpus.rs:210-237`), so punctuation and spaces prevent
  cross-boundary n-grams.

**Impact**

Configured corpus weights are not normalized probabilities. A large corpus can
dominate a smaller one even when the intended weights are equal, and changing
the size of a corpus changes its effective influence. Invalid or inconsistent
counts can produce NaNs, negative totals, or denominators that do not match the
filtered n-gram arrays. Raw-text corpora also silently lose spaces and
cross-word n-grams, which is especially relevant to H1 and to the corpus
documentation.

**Recommendation**

- Normalize each corpus to a common distribution before applying weights, or
  clearly define weights as raw-count multipliers and provide normalization
  commands.
- Validate that all counts are finite and non-negative, and recompute totals
  after filtering invalid keys.
- Validate total count relationships or at least warn when parsed files are
  internally inconsistent.
- Document exactly how raw text is tokenized, including whether spaces and
  punctuation are preserved.
- Add tests for equal-weight corpora of different sizes, negative/`NaN`
  counts, invalid keys, and parsed-file mismatch.

#### H7. Cross-hand condition rules are accepted but cannot take effect

**Evidence**

- `allowed_pair` returns `true` immediately when the two fingers are on
  different hands (`src/conditions.rs:145-155`).
- The `spread`, `separation`, and `crossing` conditions explicitly require
  `!same_hand` before they can match (`src/conditions.rs:93-95`).
- `allowed_pair` is the function used to build `pair_denied`
  (`src/conditions.rs:26-39`), so cross-hand condition rules are never
  evaluated through that path.

**Impact**

Configuration files may contain cross-hand `spread`, `separation`, or
`crossing` rules, and the parser accepts them, but they have no effect. This is
a silent semantic mismatch: users may believe they are restricting a geometry
condition when the scorer is ignoring it. The audit and reports can therefore
describe a configuration that is not actually enforced.

**Recommendation**

- Evaluate cross-hand rules before the early return, if cross-hand conditions
  are intended to be supported.
- Otherwise reject cross-hand `spread`/`separation`/`crossing` rules at config
  load time with a clear error, or remove the unsupported fields from the
  public schema.
- Add tests for each condition type with same-hand and cross-hand finger pairs.

#### H8. Generated "Reproduce" commands are incomplete and unquoted

**Evidence**

- The recorded command line is built with
  `std::env::args().collect::<Vec<_>>().join(" ")` at
  `src/generate.rs:721`. Arguments containing spaces or shell metacharacters are
  not quoted.
- The generated README writes that raw command line at
  `src/generate.rs:561-562`.
- `seed_generated` is tracked at `src/generate.rs:741`, and the seed is printed
  in output metadata, but the generated "Reproduce" command does not add
  `--seed`.
- The generated README's analysis command does use the saved layout and config
  (`src/generate.rs:563-567`), but the primary generation command remains
  incomplete.

**Impact**

A user following the generated README cannot reliably reproduce the run:
without `--seed`, a different random seed is chosen, and paths or arguments
with spaces may produce a broken or differently interpreted shell command.

**Recommendation**

- Build the reproduction command from structured arguments with shell-safe
  quoting.
- Include the effective seed explicitly when one was generated.
- Include all identity-relevant options, such as config path, corpora,
  sacrifice settings, run counts, iteration counts, and ILS settings.
- Add a golden test that parses or inspects a generated README and asserts the
  reproduce command contains the seed and safely quoted arguments.

### Medium priority

#### M1. `config.fixed.yaml` has undocumented drift from the shipped config

**Evidence**

`config/config.fixed.yaml` says that everything other than fixed fingering is
kept in sync with `config/config.yaml`, but the files differ materially. The
fixed comparison file:

- omits `sfb_impossible`;
- omits `alt_finger_penalty`;
- omits `finger_balance_source` and `finger_strength`;
- omits scissor and SFS tier settings;
- uses `hand_restrictions: [z, x, c, v]` while the shipped config uses
  `[z, x, c, v, t, y]`;
- sets `rest_penalty: 0.0`; and
- uses a different redirect representation (`redirect_scale` versus tier
  weights).

Some differences are clearly intentional for fixed-finger comparisons. Others
are not explained, and the header's synchronization claim is misleading.

**Impact**

Comparisons made with `config.fixed.yaml` may not isolate the fingering mode as
cleanly as users expect. Future changes to the shipped config can silently
diverge from the comparison config, making benchmark results non-comparable.

**Recommendation**

- Separate intentional comparison differences from incidental drift in
  documentation.
- Add a test that compares the two configurations field by field and whitelists
  only intentional differences.
- Consider generating the fixed config from the shipped config with an explicit
  transformation, or adding a checked-in "diff rationale" file.

#### M2. Configuration validation is inconsistent and order-sensitive

**Evidence**

- Unknown top-level sections are rejected, but unknown nested fields are often
  silently ignored. For example, several `parse_*` functions read known keys
  without checking that every map key is recognized.
- `as_number` validates min/max but not `NaN` or infinities
  (`src/config.rs:371-400`).
- `finger_set` accepts tokens longer than two characters such as `LPI`, using
  the last character as the finger and the first character as the hand
  (`src/config.rs:861-891`).
- `parse_finger_strength` can combine bare and hand-specific entries in a
  map-order-dependent way (`src/config.rs:1470-1509`).
- `parse_balance` averages bare and explicit readings rather than defining a
  precedence rule (`src/config.rs:1412-1467`).
- `conditions_are_default` compares raw YAML values, so reordering equivalent
  fields can change the default-detection result
  (`src/config.rs:1934-1936`).
- `alt_fingers.mode` is effectively required; an omitted mode becomes an empty
  string and fails validation (`src/config.rs:802-818`).

**Impact**

Small typos can be ignored, numeric values can poison the whole score with
NaNs, and equivalent configurations can behave differently depending on key
order. Users receive less help than the surrounding validation quality suggests.

**Recommendation**

- Reject unknown nested fields, or add a schema-driven parser with a version
  field.
- Require all numbers to be finite; use a single `finite_number` helper.
- Define and document precedence for bare versus explicit finger entries.
- Accept only exact canonical finger tokens, or explicitly document shorthand.
- Canonicalize conditions before comparing them with the default set.
- Add tests for typos in nested fields, `NaN`, infinity, token length, and
  map-order permutations.

#### M3. The model-loading API silently defaults missing parameters

**Evidence**

- `Model::from_file` starts with `PARAM_DEFAULTS` and fills only names present
  in the JSON (`src/model.rs:277-294`).
- Unknown parameter names are ignored.
- `src/model.rs:824-833` detects missing parameters, but only the config-loader
  path calls it before building the model (`src/config.rs:1865-1877`).
- `Model::p` indexes an internal map directly and can panic for an invalid name
  (`src/model.rs:298-300`).
- Parameter values are only checked for being numbers, not for finiteness.

**Impact**

Direct API users and tests can silently run a partially defaulted model,
producing results that do not correspond to any fully specified parameter file.
A typo in a parameter name is invisible, and non-finite values can propagate
through scoring.

**Recommendation**

- Make `from_file` require every parameter by default.
- Reject unknown parameter names, or return a structured report of ignored
  names.
- Validate all parameter values as finite and within sensible ranges.
- Replace the silent index in `p` with a `Result` or a checked lookup that
  cannot panic.
- Keep a separate `from_file_with_defaults` if defaulting is truly needed for
  experiments.

#### M4. Corpus parsing has ambiguous and potentially stale behavior

**Evidence**

- `Corpus::load` prefers parsed files whenever a valid parsed set exists, even
  if a newer raw source is present (`src/corpus.rs:128-145`).
- `source_file` only accepts a single matching file in a directory; multiple
  candidates return `None` (`src/corpus.rs:278-299`).
- Parsed corpus loading does not verify that input files were generated from
  the source next to them.
- Raw text counting drops spaces and punctuation, as described in H6.
- UTF-8 conversion for XZ and ZIP uses `from_utf8_lossy`.

**Impact**

Users can edit a source corpus and unknowingly keep scoring an older parsed
cache. A directory with several plausible source files produces a generic
"cannot resolve corpus" error rather than telling the user which candidates
were found. Lossy decoding can change data without an error.

**Recommendation**

- Add an explicit parsed-cache freshness policy: metadata, timestamps, or a
  `--prefer-source`/`--rebuild-parsed` flag.
- List all candidate source files in the error and require an explicit choice.
- Record the source path/hash in parsed provenance and verify it when loading.
- Reject invalid UTF-8 or document the lossy behavior.
- Add tests for stale parsed data and ambiguous source directories.

#### M5. Analysis reporting has several silent-failure and portability issues

**Evidence**

- Secondary corpus loading errors are discarded:
  `Err(_) => (None, Vec::new())` at `src/analyze.rs:1269-1276`.
- `category_report` and related report fields use heuristic classification for
  scissors/forks and other pattern categories; some fields are report-only but
  are not clearly marked as heuristic.
- `ctrl_same_finger` only checks chosen finger index `0`, and `ctrl_ring_alt`
  treats chosen left-pinky (`0`) as ring-alt without checking that the
  alternative is the ring finger on the same hand.
- `APOSTROPHE_KEY` is hard-coded as `"AC11/LSGT"`, and shifted-symbol output
  hard-codes US symbols.
- `analyze` always writes both text and JSON outputs and always prints the full
  report; there is no quiet or stdout-only mode
  (`src/analyze.rs:1248-1255`, `:1732-1735`, `:1851-1852`).

**Impact**

A typo in a secondary corpus path can quietly remove the secondary report
instead of failing. Shortcut and apostrophe advice can be wrong for non-US or
non-ANSI layouts. Scripts that only want one output format must clean up files
they did not request, and interactive low-noise use is awkward.

**Recommendation**

- Fail on secondary corpus errors unless the user explicitly requests optional
  behavior.
- Label heuristic categories clearly in JSON and text, or replace them with
  exact classifications.
- Correct the control-shortcut predicates and add unit tests for each hand and
  finger case.
- Derive apostrophe and shifted-symbol keys from geometry/config instead of
  hard-coded ANSI labels.
- Add `--quiet`, `--stdout`, and independent output-format flags, or make
  output writing opt-in.

#### M6. Search has avoidable performance costs and a constraint-bypass path

**Evidence**

- The scorer builds full tables over `N = 31` items, including a 31³ trigram
  table (`src/scorer.rs:14-21`, `:167`). This is a reasonable design but a
  significant fixed memory/initialization cost.
- `quench_generic` builds large pair/two-move/three-move collections and
  performs full-score evaluation for candidates
  (`src/search.rs:669` onward).
- `apostrophe::search_swaps` recomputes the full primary score for every letter
  pair and re-optimizes punctuation for each accepted candidate
  (`src/apostrophe.rs:190-220`).
- `search::perturb` at `src/search.rs:875` does not consult
  `Evaluator::swap_allowed`.
- `anneal` falls back to an unrestricted shuffle when
  `constrained_shuffle` fails (`src/search.rs:472-478`), which can violate hand
  restrictions in pathological configurations.
- Benchmark code exists, but no performance regression thresholds are enforced.

**Impact**

Larger search budgets and secondary objectives can become dominated by table
construction and full rescoring. The unrestricted perturb fallback can produce
invalid candidates when constraints are tight, wasting search time or producing
incorrect final states. Performance can regress without notice.

**Recommendation**

- Profile `quench_generic`, ILS, and apostrophe search before optimizing.
- Reuse move lists and incremental score state where possible; avoid rebuilding
  O(n²) candidate lists per round.
- Make `search::perturb` accept a constraint predicate or remove the generic
  unconstrained variant from production use.
- Treat `constrained_shuffle` failure as an error, or retry without violating
  restrictions.
- Add benchmark checks in CI (or a documented manual threshold) for table build,
  anneal, quench, and ILS.

#### M7. Generation has surprising side effects and incomplete artifact handling

**Evidence**

- Generation always calls `references::append_reference` for every valid row,
  even when the user did not pass `--append-references`
  (`src/generate.rs:1629-1657`). The operation mutates the configured reference
  file.
- `references::load_references(Some(&append_path)).unwrap_or_default()` at
  `src/generate.rs:1633` hides a malformed reference file in one code path,
  while other paths would return an error.
- Generated JSON is unversioned, and output formats can evolve without a schema
  marker (`src/generate.rs:1771-1825`).
- `config.yaml.bak` is written only when a layout name is supplied; partial
  output can remain after a later failure.
- Output-extension checks and default paths are inferred in several places,
  which makes scripted use brittle.
- `--ils 0` is normalized to one round by `args.ils.max(1)`
  (`src/generate.rs:1101`), and the default polish rounds is `usize::MAX`.
- `--layout` is also an alias for `--name` (`src/main.rs:151`), and some
  commands retain legacy aliases that can be confusing.

**Impact**

Running generation can modify repository data unexpectedly. A malformed
reference file can yield inconsistent behavior between the early load and the
later append. Consumers of JSON cannot detect schema changes, and failed runs
can leave directories that look complete.

**Recommendation**

- Make reference appending opt-in, or require an explicit `--append-references`
  flag with a clear default.
- Validate the existing reference file before starting the search and fail
  early if it is malformed.
- Add a `schema_version` to generated JSON and document compatibility.
- Write artifacts atomically or clean up partial output after failure.
- Clarify zero-value semantics for `--ils`, polish rounds, candidate counts,
  and runs; reject values that the implementation silently changes.
- Add command tests for default side effects and malformed reference files.

#### M8. Path resolution and XKB output are environment-dependent

**Evidence**

- `paths::repo_root()` searches current-directory ancestors for
  `config/config.yaml` and `Cargo.toml`, with a `CARGO_MANIFEST_DIR` fallback.
  Output under `layouts/` therefore depends on where the binary is invoked.
- Installed or relocated binaries may not find the repository config unless the
  current directory happens to be under the repository.
- `src/xkb.rs` hard-codes US `ROWS`, `SYMBOLS`, and extra keys, and the install
  script is fixed.
- The analysis and generation commands write under repository-relative
  directories, which may be read-only or unintended in installed deployments.

**Impact**

Running the tool from another directory can use a different config or write
artifacts somewhere unexpected. XKB output can be wrong for non-US keyboards or
non-ANSI geometry.

**Recommendation**

- Add explicit `--config` and output-root options to all commands, with a clear
  precedence order.
- Resolve the default config relative to the executable or an installation
  data directory, not only the current directory.
- Detect or require the XKB layout/geometry instead of hard-coding US symbols.
- Add tests for invocation from outside the repository and for non-US
  punctuation.

#### M9. Documentation contains claims that the implementation does not satisfy

**Evidence**

- `README.md:660` says analyzer scores reflect the constraints the generator
  applied; H1 shows that space handling and breakdown tables can differ.
- `README.md:517-519` says the balance penalty is off when `finger_balance` is
  absent from `priority_list`; H3 shows the guard remains.
- `config/config.yaml:16-18` says the search uses 100000 anneal iterations; the
  CLI default is `200_000` (`src/main.rs:161-164`). The comment also says the
  defaults live in `src/generate.rs`, while the user-facing defaults are in
  `src/main.rs`.
- `config/config.fixed.yaml` claims to keep non-fingering settings in sync; M1
  shows drift.
- `corpora/douglas/README.md` reports space counts, while the parsed trigram
  data used by the scorer contains no spaces.
- The generated README's reproduce command omits the generated seed and does
  not quote arguments; see H8.

**Impact**

Users and maintainers can make decisions from documentation that no longer
matches behavior. This is particularly damaging for scoring semantics and
comparison claims.

**Recommendation**

- Update the claims above after fixing or explicitly documenting the behavior.
- Add documentation tests or scripted checks for default values and generated
  commands.
- Keep corpus statistics generated from the parsed data rather than maintained
  by hand.
- Add a short "known limitations" section covering space behavior, heuristic
  reports, fixed-config differences, and supported platforms.

#### M10. Test coverage misses the highest-risk paths

**Evidence**

The current suite is green but does not cover:

- space-bearing corpora across `score_layout`, `Evaluator`, and
  `score_breakdown`;
- duplicate/missing/Unicode layout validation;
- negative, `NaN`, infinite, and oversized time budgets;
- balance semantics when `finger_balance` is absent from `priority_list`;
- cross-hand condition rules;
- malformed/oversized ZIP, XZ, and JSON corpora;
- multiple corpus weights and normalization;
- generated README reproducibility and command quoting;
- non-US XKB output; and
- config nested-field typos and non-finite numbers.

The baseline and parity tests also depend on fixed expected strings and stored
scores, which makes some regressions obvious but does not exercise these
edge-case contracts. The benchmark has no assertions or CI threshold.

**Recommendation**

- Add focused unit tests for each high- and medium-priority finding.
- Add property/fuzz tests for config parsing, corpus parsing, and layout
  validation.
- Add a synthetic space corpus and an explicit no-space corpus to make H1's
  intended semantics clear.
- Add a generated-artifact test that reads the README and verifies the complete
  reproduce command.
- Add a small CI matrix (Linux at minimum, optionally macOS) with formatting,
  Clippy, tests, and the Python import check.

#### M11. Development tooling and repository hygiene are incomplete

**Evidence**

- There is no `.github/` CI workflow.
- There is no root `LICENSE` file despite the crate declaring MIT in
  `Cargo.toml`.
- Corpus licenses and source attribution are not consistently documented,
  especially for the Discord/community-derived corpus.
- There is no `rust-toolchain` file, so builds depend on the developer's
  installed compiler.
- Python development tools have no pinned requirements or environment file.
- `target/` is locally 1.3G and `.venv/` is 127M; both are ignored, but a stale
  copied `target/` caused the false test failures noted above.
- The root `README.md` and several files are owned by a different user than the
  repository tree, which can cause permission friction in shared environments.

**Impact**

A fresh contributor cannot reproduce the exact toolchain or Python environment
from repository metadata alone. Licensing and corpus provenance are unclear,
and stale build artifacts can waste significant time and produce misleading
test failures.

**Recommendation**

- Add a CI workflow for fmt, Clippy, tests, and the development-tool import
  check.
- Add a root license and a corpus attribution/licensing section.
- Add `rust-toolchain.toml` and a Python requirements/lock file if the tooling
  is supported.
- Add a clean-build check or document the required `CARGO_TARGET_DIR`
  isolation.
- Normalize repository file ownership/permissions if the project is shared.

### Lower-risk observations

- `src/words.rs` accepts `--min-frequency` as an unvalidated `f64`; `NaN` makes
  the filter comparisons false and includes everything.
- Wordlist output and `--limit 0` behavior are only lightly tested.
- `latest` layout handling uses a symlink and can leave stale or partial state
  if a run is interrupted.
- Some report-only classifications are heuristic and should be labeled as such
  even if they are not used in scoring.
- `score_breakdown` includes a `space` field, but the default corpora cannot
  exercise it; this is a testing and documentation gap as well as the semantic
  issue in H1.
- Error messages in some paths use `unwrap_or_default()` for report fields,
  which can hide missing data.
- The project has a good audit layer, but audit coverage should be extended
  alongside the new tests for spaces, cross-hand conditions, and corpus
  weighting.

## Suggested roadmap

### Phase 1: correctness and safety

1. Fix space representation and make `score_layout`, `Evaluator`, and
   `score_breakdown` agree; add space-aware tests.
2. Fix balance-guard semantics and update the README and fixtures.
3. Harden layout validation and remove byte-indexed grid slicing.
4. Validate budgets and other numeric CLI inputs.
5. Make cross-hand condition rules either work or fail at config load.

### Phase 2: data and reproducibility

6. Add corpus size limits, UTF-8 validation, archive member selection rules, and
   normalization tests.
7. Define corpus weighting semantics and validate counts/totals.
8. Fix generated reproduction commands, shell quoting, seed inclusion, and JSON
   schema versioning.
9. Make generation side effects explicit and artifact writes atomic.

### Phase 3: performance, tooling, and documentation

10. Profile and optimize `quench_generic`, ILS, and apostrophe search.
11. Reconcile `config.fixed.yaml` with `config.yaml` and test intentional
    differences.
12. Add CI, a toolchain file, Python dependency metadata, and a root license.
13. Update README/config/corpus documentation and add generated-command tests.

## Verification checklist for future changes

Run:

```sh
cargo fmt --check
CARGO_TARGET_DIR=/tmp/opencode/shirolyzer-target cargo clippy --all-targets -- -D warnings
CARGO_TARGET_DIR=/tmp/opencode/shirolyzer-target cargo test --release
```

When changing scoring or corpus behavior, also add and run:

- a synthetic space corpus test;
- a breakdown reconstruction test with and without spaces;
- a balance-priority omission test;
- malformed/oversized corpus tests;
- generated README/command-line tests; and
- a clean-checkout build after removing any copied `target/` directory.

## Conclusion

The project is in good shape at the module and test-infrastructure level. The
highest-value work is to make the scoring model internally consistent, remove
silent input-validation gaps, and harden corpus ingestion. Those changes will
make the existing strong test suite much more trustworthy and will prevent
latent issues from becoming user-visible as soon as space-aware or externally
sourced corpora are used.
