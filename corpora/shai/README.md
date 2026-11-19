# Shai's Cleaned iWeb corpus (90m words)

Source: [AKL (Alt Keyboard Layouts) Discord server](https://discord.gg/kXyGkVaez)
Direct Download Link: <https://colemak.com/pub/corpus/iweb-corpus-samples-cleaned.txt.xz>
License: no explicit license is included with the sample. The bundled parsed
counts are derived data from the community sample; confirm the upstream terms
before redistributing the raw corpus.
Attribution: credit Shai and the AKL community, and note the underlying iWeb
sample when publishing results derived from it.

## Build

```sh
python3 corpora/shai/parse_shai.py            # download, parse, write parsed/
python3 corpora/shai/parse_shai.py --check    # download, parse, compare only
python3 corpora/shai/parse_shai.py --source FILE.xz [--keep-source]
```

The parser downloads the `.xz` source to a temporary file, streams the
decompression, and deletes the downloaded file on success. `--source` uses an
existing local `.txt` or `.txt.xz` file instead, and `--keep-source` keeps a
downloaded archive for debugging.

Output: `corpora/shai/parsed/letter_freq.json`,
`corpora/shai/parsed/bigram_freq.json`, and
`corpora/shai/parsed/trigram_freq.json`.  Entries are ordered from most to
least frequent (ties by key), and the parser reports which files it changed
or that the parsed data was already unchanged.

## Totals

Generated from the bundled `parsed/*.json` files:

```text
Letters:  418,381,828
Bigrams:  327,253,737
Trigrams: 240,524,895
```

The parser also counts 91,128,091 raw `[a-z]+` runs while scanning; run counts
are not stored in the parsed JSON.

Processing: lowercase, maximal `[a-z]+` runs; non-letters break runs.
