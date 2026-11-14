# Configuration

`config/config.yaml` is the active runtime configuration. Relative paths inside
a config file are resolved from this directory.

- `config.yaml` — shipped flexible-fingering configuration.
- `config.fixed.yaml` — fixed-fingermap comparison configuration.
- `geometry/` — physical keyboard geometry files. `geometry/ansi.yaml` is the
  shipped ANSI geometry.
- `model/` — fitted model parameters. `model/calibrated.json` is the live model.
- `references.toml` — named reference layouts and stored comparison scores.
- `score_baseline.json`, `rulings.json` — regression baselines and audit rulings.
- `experiments/` — alternate configs and seed lists for controlled experiments.
- `preference_variants.yaml`, `redirect_variants.yaml` — sweep inputs.

`generate --layout <name>` copies the active config to
`layouts/<name>/config.yaml.bak`. The backup keeps the same relative paths and
is intended to be reusable with `--config`.
