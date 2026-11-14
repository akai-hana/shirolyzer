# Layout artifacts

`layouts/` is the output root. `layouts/shiro/` is the curated shipped layout
and is tracked; other generated directories are ignored by default.

`generate --layout <name>` creates `layouts/<name>/` with:

- `layout.txt` — selected 30-character layout.
- `generate.txt`, `generate.json` — full generation record.
- `analysis.txt`, `analysis.json` — analysis under the saved config.
- `config.yaml.bak` — exact config snapshot used for the run.
- `README.md` — generated summary and reproduction command.
- `xkb/<name>`, `xkb/install.sh` — XKB symbols and installer.

`latest` is a local convenience symlink to the current layout file. It is
ignored by git; the shipped layout is always at `layouts/shiro/layout.txt`.

To keep another generated layout in the repository, add an explicit
`!/layouts/<name>/` rule to `.gitignore` and review the files before committing.
