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

## Source attribution and licensing

The repository's MIT license covers the shirolyzer code, not the bundled corpus
data. Each corpus keeps its own upstream provenance and terms.

### Douglas (Spanish)

- Source: Ian Douglas, "Creating a Corpus and Chained Bigrams for Spanish
  Keyboard Development and Evaluation", version 1.0.0, Zenodo (2021),
  [doi:10.5281/zenodo.5501931](https://doi.org/10.5281/zenodo.5501931).
- License: [Creative Commons Attribution 4.0
  International](https://creativecommons.org/licenses/by/4.0/) (CC BY 4.0).
- The parsed counts are derived from `clean-spanish-book-extracts.txt` in that
  record. The bundled PDF is the companion report (version 1.0.1).
- Attribution: © 2021 Ian Douglas. Cite the DOI when reusing the corpus.

### Shai (English)

- Source: Shai's cleaned iWeb sample (90m words), shared with the Alt Keyboard
  Layouts (AKL) community and mirrored at
  <https://colemak.com/pub/corpus/iweb-corpus-samples-cleaned.txt.xz>.
- License: no explicit license is included with the sample. The bundled parsed
  counts are derived data from that community sample; confirm the upstream
  terms before redistributing the raw corpus.
- Attribution: credit Shai and the AKL community, and note the underlying iWeb
  sample when publishing results derived from it.
