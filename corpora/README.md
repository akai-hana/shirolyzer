# Corpora

- `shai/` — English iWeb sample used as the primary corpus.
- `douglas/` — Spanish book corpus used as the secondary corpus.

Each corpus directory contains its parser, source notes, and the parsed
`letter_freq.json`, `bigram_freq.json`, and `trigram_freq.json` files. The Rust
runtime loads the parsed files; the parsers rebuild them from the upstream
archives.

```sh
python3 corpora/shai/parse_shai.py --check
python3 corpora/douglas/parse_douglas.py --check
```

The runtime accepts parsed JSON directories or raw `.txt`/`.xz`/`.zip`
sources. A corpus directory must contain exactly one raw source, and a ZIP
must contain exactly one `.txt` member. Archives are capped at 64 MiB,
decompressed/raw text at 256 MiB, and each JSON file at 64 MiB; all text must
be valid UTF-8.
