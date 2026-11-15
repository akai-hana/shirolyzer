# Roll-scale, SFS-tier, and residual redirect audit

Date: 2026-11-15

## Question

The shipped objective puts rolls above alternation and finger balance. If roll
credit is lowered, do the freed points go somewhere useful (SFB/SFT, scissors,
redirects, common-word comfort), or does the layout merely lose rolls and get
worse? The same audit also checks whether the residual redirects and SFS in
shiro are already the mild kinds.

## New knob

`roll_scale` is a top-level config key (default `1.0`). It multiplies roll
credit only:

```yaml
roll_scale: 0.5
```

Leap penalties stay attached to the `rolls` priority, so lowering
`priority_list`'s `rolls` multiplier still scales both rolls and leaps. This
makes `roll_scale` the clean knob for a roll-only sweep. `priority_summary`
prints the effective roll multiplier so a run records the change.

## Sweep

Variants from `config/preference_variants.yaml`:

```text
baseline, roll-scale-075, roll-scale-050, roll-scale-025, roll-off
```

Command:

```sh
.venv/bin/python utils/preference_sweep.py \
  --variants /tmp/roll-sweep-variants.yaml \
  --start-seed 0 --seeds 2 \
  --runs 4 --iters 60000 --ils 2 --ils-candidates 2 \
  --secondary-rounds 1 --polish-rounds 1
```

The base config was re-applied to every selected layout before reading the
scores below, so the primary/secondary columns are comparable across variants.

| variant | seed | layout | base primary | base secondary |
|---|---:|---|---:|---:|
| baseline | 0 | `ftrhcguenjsvld'oawpimzxky,.bq/` | 4.506751 | 4.658233 |
| baseline | 1 | `mhrcvguenjtdls,oawpifxzyk./bq'` | 4.530157 | 4.726465 |
| roll-scale-075 | 0 | `mhrcgfuenwtdls,oajpivxzyk./bq'` | 4.550296 | 4.706974 |
| roll-scale-075 | 1 | `ftrhcguenwsvld'oajpimzxky,.bq/` | 4.521629 | 4.652739 |
| roll-scale-050 | 0 | `vtrgdwuenfsplc,oijhamzxyk./bq'` | 4.519339 | 4.522552 |
| roll-scale-050 | 1 | `vtrgdwuenfsplc,oijhamzxyk./bq'` | 4.519339 | 4.522552 |
| roll-scale-025 | 0 | `mhrcvguenwtdls,oajpifxzyk./bq'` | 4.544747 | 4.720933 |
| roll-scale-025 | 1 | `ftrgdwuenbsmlc,oajhivzxyk./pq'` | 4.504472 | 4.627863 |
| roll-off | 0 | `ftrhcguenwsvld,oijpamzxky./bq'` | 4.528400 | 4.509850 |
| roll-off | 1 | `ftrhcguenwsvld,oijpamzxky./bq'` | 4.528400 | 4.509850 |

Motion shares for the distinct outcomes (shipped config, chosen fingering):

| layout | roll tier 1 % | leap % | SFS true % | redirects % | tier 6 % | unclassified % |
|---|---:|---:|---:|---:|---:|---:|
| shipped | 7.999 | 8.227 | 2.884 | 3.784 | 0.147 | 0.187 |
| roll-050 `vtrgd...` | 5.490 | 8.958 | 3.123 | 4.336 | 0.065 | 0.280 |
| roll-025 `ftrgd...` | 5.958 | 8.284 | 3.192 | 3.853 | 0.147 | 0.198 |
| roll-off `ftrh...` | 6.547 | 8.797 | 3.181 | 4.072 | 0.065 | 0.234 |

## Reading the results

- Lowering roll credit does what it says: tier-1 roll share falls from 8.0% to
  5.5–6.5%, and the search reaches different letter placements.
- There is no consistent full-objective win. The best alternative
  (`roll-scale-025`, seed 1) is only 0.00228 primary points better (0.05%) and
  0.030 secondary points better, while paying 0.31 percentage points more SFS,
  more unclassified redirects, and worse finger balance.
- A stronger rerun of the same setting (`8 × 80k`, ILS 3, polish 2) did not
  reproduce that layout; it returned a 4.522 primary layout. The candidate is
  therefore a low-budget basin, not evidence that the roll term is mis-tuned.
- The `roll-scale-050` and `roll-off` layouts improve the Douglas secondary
  score (by 0.136 and 0.148) but cost 0.013 and 0.022 primary points. That is a
  deliberate secondary-for-primary trade, not a free improvement.
- The earlier shortcut check flagged `v` on `Z` for `roll-scale-025` seed 1.
  The corrected check treats a shortcut key on a pinky physical key as
  ring-altable (`ctrl_ring_alt=true`), so that is no longer a rejection. The
  candidate still fails to reproduce and loses on SFS, unclassified redirects,
  and finger balance.

Decision: keep `rolls` at its current priority. `roll_scale` stays as an
experimental knob and as a search-diversification option; it is not a reason to
change the shipped config.

## Does the model differentiate SFS?

Only by physical distance. Each same-finger skipgram pays

```text
W_SFS_1U  = 2.50   when the outer keys are <= 1.3u apart
W_SFS_FAR = 3.50   otherwise
```

multiplied by the `sfbs-sfts` priority and `general.sfs_scale`. There is no
per-finger-pair ladder analogous to `redirect_tiers`, and `analyze` reports one
aggregate `sfs_true` share. Index-middle-index, middle-ring-middle, and
ring-pinky-ring skipgrams are not separated at scoring time unless a condition
forbids a specific pattern.

The layout review in `common_words_best_layout.md` already used a manual
index-middle vs ring-pinky SFS split (63.0/37.0 for the shipped layout). That
is an analysis-time quality gate, not a runtime cost. This gap motivated the
`sfs_tiers` implementation described next.

## SFS tier experiment

`sfs_tiers` is an ordered list of per-hand rank patterns, mirroring
`redirect_tiers`. `sfs_tier_weights` are multipliers on the existing
distance-dependent base SFS cost, not absolute costs. The repeated outer finger
fixes the hand; a cross-hand middle alt contributes its finger rank. The four
same-rank triples (`1-1-1` … `4-4-4`) represent a cross-hand middle of the same
finger type, while three presses of one physical finger are an SFT and never
reach the SFS classifier. A full table therefore has 16 patterns. Unlisted
patterns keep the legacy multiplier `1.0`.

The analyzer reports repeat-free `sfs_tier_N`, per-pattern `sfs_A_B_C`, and
`sfs_unlisted`; these use the same repeat-free set as `sfs_true`. The legacy
`sfs` category still includes repeated keys.

Fixed-layout results on the shipped letters (Shai primary, Douglas secondary):

| config | primary | secondary | SFS true % | tier 1 % | tier 2 % | tier 3 % | tier 4 % | tier 5 % | unlisted % |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| shipped | 4.506751 | 4.658233 | 2.884 | — | — | — | — | — | — |
| `sfs-tiers` | 4.508708 | 4.669376 | 2.457 | 0.758 | 1.677 | 0.022 | 0.000 | 0.000 | 0.000 |
| `sfs-tiers-neutral` | 4.505864 | 4.665966 | 2.677 | 0.758 | 1.897 | 0.022 | 0.000 | 0.000 | 0.000 |
| `sfs-tiers-steep` | 4.547971 | 4.720283 | 1.991 | 0.708 | 1.279 | 0.004 | 0.000 | 0.000 | 0.000 |

`config/preference_variants.yaml` contains the three variants. `sfs-tiers`
places `3-3-3` in tier 4 and `4-4-4` in tier 5; `sfs-tiers-neutral` keeps all
same-rank triples in tier 2, which is the neutral multiplier under the standard
weights.

- The full standard ladder is a small primary regression (+0.001957) but a
  secondary improvement (+0.011143) and cuts SFS true by 0.427 percentage
  points. It is a trade, not a free win.
- Moving the same-rank triples to the neutral tier flips primary to a tiny
  improvement (-0.000887) and keeps most of the secondary gain (+0.007733),
  with SFS true down 0.207 points. The result is sensitive to where cross-hand
  same-rank patterns sit, so that placement is a calibration decision, not a
  settled result.
- The steep ladder is expensive on primary (+0.041220) even though it improves
  secondary (+0.062050) and pushes SFS true down to 1.991%. Like the steep
  redirect sweep, it mostly changes chosen fingering; applying the same severe
  multipliers to cross-hand SFS over-penalizes alts that the model already
  charges for travel.

Decision: keep the rank-based 16-pattern cross-hand classification. The outer
finger fixes the hand, a cross-hand middle contributes its rank, and the four
same-rank triples represent a cross-hand middle of the same finger type. Keep
`sfs_tiers` out of the shipped config for now: the standard ladder is close to
neutral, but the steep ladder is not a free improvement and the table still
needs a full search comparison.

## Residual redirects

The shipped layout still contains grave redirects, so the remaining ones are
not all mild. On Shai, tier 6 is 0.147% of trigrams and unclassified turns are
0.187%; on the common-word corpus, tier 6 is 0.116% and unclassified 0.167%.
Examples from the common-word worst list:

| trigram | fingers | pattern | tier | occurrences |
|---|---|---|---:|---:|
| `ien` | RP-RM-RR | 4-2-3 | 6 | 1,102,502 |
| `sts` | LP-LR-LP | 4-3-4 | 6 | 186,058 |
| `nei` | RR-RM-RP | 3-2-4 | 6 | 127,447 |
| `bui` | RM-RI-RR | 2-1-3 | 4 | 167,202 |
| `ryt` | LM-LI-LR | 2-1-3 | 4 | 520,487 |
| `rty` | LM-LR-LI | 2-3-1 | unclassified | 298,731 |
| `yst` | LI-LP-LR | 1-4-3 | unclassified | 141,223 |
| `pie` | RR-RP-RM | 3-4-2 | unclassified | 191,464 |

Unclassified turns are charged the worst configured tier (5.0), so they are
penalized, not ignored. The common-word redirect total is 3.58%, but most of it
is tier 1–3; roughly 0.88% is tier 4 + tier 6 + unclassified. The most visible
residual is `ien` (4-2-3) in words such as `experience`, `client`, and
`science`.

A steep redirect sweep (`redirect_tier_weights: [0, 0.10, 0.20, 3.0, 5.0, 7.0]`)
did not change the selected layout at this budget, but it did change chosen
fingering: the analyzer stopped taking the 4-2-3 route for `ien`, tier 6 fell
from 0.147% to 0.0015%, and unclassified redirects dropped from 0.187% to
0.096% on Shai. So part of the residual grave-redirect share is a fingering
choice, not a letter-placement constraint. Steepening tier 6 trades those
redirects for more alt/cross-hand work; it does not make the layout free of
them.

## Moderate tier-6 redirect bump

The steep redirect profile showed that the analyzer can re-finger the 4-2-3
`ien` route. A smaller bump should remove more grave redirects than the shipped
`5.0` without paying the full steep cost. Fixed-layout results:

| tier-6 weight | primary | secondary | SFS true % | redirects % | tier 6 % | unclassified % |
|---:|---:|---:|---:|---:|---:|---:|
| shipped `5.0` | 4.506751 | 4.658233 | 2.884 | 3.784 | 0.147 | 0.187 |
| `5.5` | 4.509491 | 4.667067 | 2.906 | 3.745 | 0.104 | 0.125 |
| `6.0` | 4.510627 | 4.668959 | 3.029 | 3.739 | 0.001 | 0.099 |
| `6.5` | 4.511626 | 4.670653 | 3.030 | 3.739 | 0.001 | 0.097 |
| `7.0` | 4.512610 | 4.672327 | 3.031 | 3.739 | 0.001 | 0.096 |

`6.0` is the knee: it is the smallest bump that makes the 4-2-3 `ien` route
disappear, while `5.5` still leaves 0.104% tier 6. The cost is 0.145 percentage
points more SFS true (2.884 → 3.029) and 0.003876 more primary. Redirects fall
by 0.045 points and unclassified turns by 0.088 points, and Douglas secondary
improves by 0.010726. That is not a full-objective win, so the shipped
`[0.0, 0.05, 0.10, 2.0, 3.5, 5.0]` stays.

## Alternative layout: primary gain, secondary cost

A short sweep under the SFS and tier-6 variants found
`ftrhcguenbsmld'oawpivqxyz,.kj/`. Re-analyzed with the base config it scores
primary `4.487234` (better by 0.019517) and secondary `4.557580` (worse by
0.100653), with SFS true at `3.100%` and redirects at `3.840%`. Under the new
calibration (`sfs-tiers` + tier-6 `6.0`) it scores primary `4.493212` versus
`4.512604` for the shipped layout, and secondary `4.573585` versus `4.680358`.
It is therefore a primary-for-secondary trade, not a full-objective win.

The earlier shortcut check incorrectly flagged `v` on the left-pinky `Z` key.
The corrected check treats that shortcut as ring-altable, so the layout is no
longer rejected on a guardrail. It is still not adopted, because its secondary
score, SFS share, and redirect share are worse.

## Newly computed layout under the new calibration

The new calibration was `sfs-tiers` (standard rank-based 16-pattern table) plus
`redirect_tier_weights: [0.0, 0.05, 0.10, 2.0, 3.5, 6.0]`. A focused generate
run (8 runs, 100k iterations, ILS 3, 4-minute budget) seeded from the shipped
layout returned the shipped layout: internal primary `4.5715`, secondary
`4.9660`; analyze under the new calibration gives `4.512604` / `4.680358`.

Seeding the same search from the alternative layout produced a different valid
layout:

```text
m h r c v  g u e n j
 t d l s ,  o a w p i
  f x z y k  . / b q '
```

Base config: primary `4.530157`, secondary `4.726465`; new calibration:
primary `4.532668`, secondary `4.730518`. The layout improves the Douglas
secondary score relative to the shipped layout but loses about 0.02 primary
points, and it is worse than the alternative on primary. The search either
returns the shipped layout or trades primary for secondary; it did not produce
a full-objective winner under this calibration.

A second campaign with 16 random starts and no seed layout (6-minute budget)
converged to the same layout (internal primary `4.5681`). The random starts
confirm that this is the calibration's preferred basin rather than an artifact
of the alternative seed. The search's internal primary is not the analyzer
score: it ranks this layout slightly ahead of the shipped layout (`4.5715`
internal), while `analyze` ranks it behind (`4.532668` vs `4.512604`). The
analyzer values remain the comparison of record.

## Calibration options

1. Keep the rank-based 16-pattern cross-hand policy. Before shipping a table,
   run a full search comparison on both corpora: the standard ladder is
   near-neutral on the fixed layout, the placement of same-rank cross-hand
   triples changes the result, and the steep ladder over-penalizes cross-hand
   alts. A separate cross-hand scale is still an option if the sweep shows the
   rank-based multipliers are too blunt.
2. Keep tier 6 at `5.0`. A moderate bump of `6.0` removes the 4-2-3 `ien`
   route but costs primary and SFS, so it is a deliberate redirect-for-comfort
   trade rather than a calibration win. If it is pursued, search letters under
   the `6.0` config instead of only re-fingering the shipped layout.
3. Keep `roll_scale` as a search-diversification parameter. It produced real
   layout diversity, but no repeatable full-objective improvement at the tested
   budgets.
4. Keep the corrected `Ctrl` shortcut model: left `Ctrl` is `LP`, and a
   shortcut key on a pinky physical key can use that hand's ring finger, so it
   is not a same-finger conflict. Re-check other external guardrails before
   accepting any layout that improves the modeled score.

## Reproduction

The sweep variants are kept in `config/preference_variants.yaml`. Re-run a
focused subset with `utils/preference_sweep.py` and the same flags, then
re-analyze every selected layout with `config/config.yaml` before comparing
scores. The fixed-layout SFS and tier-6 rows were produced by applying the
named override to `config/config.yaml` and running `analyze` on
`layouts/shiro/layout.txt`; no letters were changed. The new-calibration
layout used a merged `sfs-tiers` + `redirect-tier6-6` config and `generate`
with `--runs 8 --iters 100000 --ils 3 --ils-candidates 3 --secondary-rounds 2
--polish-rounds 2 --time-budget 4m`, seeded once from the shipped layout and
once from the alternative. The from-scratch confirmation used the same merged
config and flags with `--seed 0 --runs 16 --time-budget 6m`, no seed layout,
and no reference padding.
