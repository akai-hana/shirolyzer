# Python utilities

These modules are development tooling only. The shipped runtime is the Rust
binary in `src/`.

- `config.py`, `comfort.py`, `conditions.py`, `gates.py`, `rulings.py` — Python
  parity and model-inspection helpers.
- `ansi/` — ANSI physical layout definitions.
- `fitlib.py` — shared fitting helpers used by `fitting/`.
- `compute_variance.py` — run-to-run `generate` variance measurement.
- `preference_sweep.py` — config-variant sweep driver.
- `paths.py` — repository-relative path helpers.

Run `just dev-tools` to verify the retained Python modules still import.
