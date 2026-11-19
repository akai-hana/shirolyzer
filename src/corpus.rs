//! N-gram corpus loading (parsed JSON or raw text sources).

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

pub const SPACE_INDEX: usize = 26;
pub(crate) const PARSED_FILES: [&str; 3] =
    ["letter_freq.json", "bigram_freq.json", "trigram_freq.json"];

const MIB: u64 = 1024 * 1024;
pub const MAX_COMPRESSED_BYTES: u64 = 64 * MIB;
pub const MAX_DECOMPRESSED_BYTES: u64 = 256 * MIB;
pub const MAX_JSON_BYTES: u64 = 64 * MIB;

#[derive(Clone, Debug, Default)]
pub struct Corpus {
    pub letters: HashMap<char, f64>,
    pub bigrams: HashMap<(char, char), f64>,
    pub trigrams: HashMap<(char, char, char), f64>,
    pub bi_ids: Vec<[usize; 2]>,
    pub bi_counts: Vec<f64>,
    pub tri_ids: Vec<[usize; 3]>,
    pub tri_counts: Vec<f64>,
    pub total_letters: f64,
    pub total_bigrams: f64,
    pub total_trigrams: f64,
}

fn cid(ch: char) -> Option<usize> {
    match ch {
        'a'..='z' => Some(ch as usize - 'a' as usize),
        ' ' => Some(SPACE_INDEX),
        _ => None,
    }
}

impl Corpus {
    pub fn new(
        letters: HashMap<char, f64>,
        bigrams: HashMap<(char, char), f64>,
        trigrams: HashMap<(char, char, char), f64>,
    ) -> Self {
        let letters: HashMap<char, f64> = letters
            .into_iter()
            .filter(|(ch, _)| ch.is_ascii_lowercase())
            .collect();
        let bigrams: HashMap<(char, char), f64> = bigrams
            .into_iter()
            .filter(|((a, b), _)| cid(*a).is_some() && cid(*b).is_some())
            .collect();
        let trigrams: HashMap<(char, char, char), f64> = trigrams
            .into_iter()
            .filter(|((a, b, c), _)| cid(*a).is_some() && cid(*b).is_some() && cid(*c).is_some())
            .collect();

        let mut letter_keys: Vec<char> = letters.keys().copied().collect();
        letter_keys.sort_unstable();
        let total_letters = letter_keys.iter().map(|k| letters[k]).sum();
        let mut bigram_keys: Vec<(char, char)> = bigrams.keys().copied().collect();
        bigram_keys.sort_unstable();
        let total_bigrams = bigram_keys.iter().map(|k| bigrams[k]).sum();
        let mut trigram_keys: Vec<(char, char, char)> = trigrams.keys().copied().collect();
        trigram_keys.sort_unstable();
        let total_trigrams = trigram_keys.iter().map(|k| trigrams[k]).sum();
        let mut bi_ids = Vec::with_capacity(bigram_keys.len());
        let mut bi_counts = Vec::with_capacity(bigram_keys.len());
        for &(a, b) in &bigram_keys {
            if let (Some(ai), Some(bi)) = (cid(a), cid(b)) {
                bi_ids.push([ai, bi]);
                bi_counts.push(bigrams[&(a, b)]);
            }
        }
        let mut tri_ids = Vec::with_capacity(trigram_keys.len());
        let mut tri_counts = Vec::with_capacity(trigram_keys.len());
        for &(a, b, c) in &trigram_keys {
            if let (Some(ai), Some(bi), Some(ci)) = (cid(a), cid(b), cid(c)) {
                tri_ids.push([ai, bi, ci]);
                tri_counts.push(trigrams[&(a, b, c)]);
            }
        }
        Self {
            letters,
            bigrams,
            trigrams,
            bi_ids,
            bi_counts,
            tri_ids,
            tri_counts,
            total_letters,
            total_bigrams,
            total_trigrams,
        }
    }

    /// Normalize each n-gram order to a unit total. The score is a ratio, so
    /// this does not change a single corpus's score; it makes configured
    /// weights act as mixture shares instead of raw-count multipliers.
    pub fn normalized(&self) -> Self {
        Self::new(
            normalize_map(&self.letters),
            normalize_map(&self.bigrams),
            normalize_map(&self.trigrams),
        )
    }

    pub fn scaled(&self, factor: f64) -> Result<Self> {
        if !factor.is_finite() || factor < 0.0 {
            return Err(anyhow!(
                "corpus scale factor must be finite and non-negative, got {factor}"
            ));
        }
        if factor == 1.0 {
            return Ok(self.clone());
        }
        let corpus = Self::new(
            self.letters.iter().map(|(k, v)| (*k, v * factor)).collect(),
            self.bigrams.iter().map(|(k, v)| (*k, v * factor)).collect(),
            self.trigrams
                .iter()
                .map(|(k, v)| (*k, v * factor))
                .collect(),
        );
        corpus.validate_totals()?;
        Ok(corpus)
    }

    fn validate_totals(&self) -> Result<()> {
        for (name, total) in [
            ("letter", self.total_letters),
            ("bigram", self.total_bigrams),
            ("trigram", self.total_trigrams),
        ] {
            if !total.is_finite() || total < 0.0 {
                return Err(anyhow!(
                    "corpus {name} total must be finite and non-negative, got {total}"
                ));
            }
        }
        Ok(())
    }

    pub fn merge(corpora: &[Corpus]) -> Result<Self> {
        if corpora.is_empty() {
            return Err(anyhow!("no corpora to merge"));
        }
        if corpora.len() == 1 {
            return Ok(corpora[0].clone());
        }
        let mut letters = HashMap::new();
        let mut bigrams = HashMap::new();
        let mut trigrams = HashMap::new();
        for corpus in corpora {
            let mut letter_keys: Vec<char> = corpus.letters.keys().copied().collect();
            letter_keys.sort_unstable();
            for k in letter_keys {
                *letters.entry(k).or_insert(0.0) += corpus.letters[&k];
            }
            let mut bigram_keys: Vec<(char, char)> = corpus.bigrams.keys().copied().collect();
            bigram_keys.sort_unstable();
            for k in bigram_keys {
                *bigrams.entry(k).or_insert(0.0) += corpus.bigrams[&k];
            }
            let mut trigram_keys: Vec<(char, char, char)> =
                corpus.trigrams.keys().copied().collect();
            trigram_keys.sort_unstable();
            for k in trigram_keys {
                *trigrams.entry(k).or_insert(0.0) += corpus.trigrams[&k];
            }
        }
        let merged = Self::new(letters, bigrams, trigrams);
        merged.validate_totals()?;
        Ok(merged)
    }

    pub fn load(path: &Path) -> Result<Self> {
        if let Some(parsed) = parsed_dir(path) {
            return Self::load_parsed(&parsed);
        }
        if let Some(source) = source_file(path)? {
            let text = read_source(&source)?;
            let normalized: String = text
                .to_lowercase()
                .nfkd()
                .filter(|c| !is_combining_mark(*c))
                .collect();
            return Ok(Self::count_text(&normalized));
        }
        Err(anyhow!(
            "cannot resolve corpus {}: expected a parsed directory, a corpus directory containing parsed/, or a raw .txt/.xz/.zip source",
            path.display()
        ))
    }

    fn load_parsed(dir: &Path) -> Result<Self> {
        let (letters_raw, (bigrams_raw, trigrams_raw)) = rayon::join(
            || read_json::<HashMap<String, f64>>(&dir.join(PARSED_FILES[0])),
            || {
                rayon::join(
                    || read_json::<HashMap<String, f64>>(&dir.join(PARSED_FILES[1])),
                    || read_json::<HashMap<String, f64>>(&dir.join(PARSED_FILES[2])),
                )
            },
        );
        let corpus = Self::from_raw_maps(letters_raw?, bigrams_raw?, trigrams_raw?)
            .with_context(|| format!("invalid corpus counts in {}", dir.display()))?;
        if corpus.total_letters == 0.0 {
            return Err(anyhow!(
                "parsed corpus {} has no supported letter counts",
                dir.display()
            ));
        }
        if corpus.total_bigrams == 0.0 && corpus.total_trigrams > 0.0 {
            return Err(anyhow!(
                "parsed corpus {} has trigrams but no bigram counts",
                dir.display()
            ));
        }
        Ok(corpus)
    }

    /// Load an external comparison bundle: a JSON object with `letters`,
    /// `bigrams`, and `trigrams` maps from n-gram strings to counts.
    pub fn load_external(path: &Path) -> Result<Self> {
        #[derive(serde::Deserialize)]
        struct ExternalCorpus {
            letters: HashMap<String, f64>,
            bigrams: HashMap<String, f64>,
            trigrams: HashMap<String, f64>,
        }
        let payload: ExternalCorpus = read_json(path)
            .with_context(|| format!("expected letters/bigrams/trigrams in {}", path.display()))?;
        Self::from_raw_maps(payload.letters, payload.bigrams, payload.trigrams)
            .with_context(|| format!("invalid counts in {}", path.display()))
    }

    fn from_raw_maps(
        letters_raw: HashMap<String, f64>,
        bigrams_raw: HashMap<String, f64>,
        trigrams_raw: HashMap<String, f64>,
    ) -> Result<Self> {
        let mut letters = HashMap::new();
        for (k, v) in letters_raw {
            let Some(ch) = single_char(&k) else {
                continue;
            };
            if !ch.is_ascii_lowercase() {
                continue;
            }
            validate_count("letter", &k, v)?;
            letters.insert(ch, v);
        }
        let mut bigrams = HashMap::new();
        for (k, v) in bigrams_raw {
            let chars: Vec<char> = k.chars().collect();
            if chars.len() != 2 || chars.iter().any(|ch| cid(*ch).is_none()) {
                continue;
            }
            validate_count("bigram", &k, v)?;
            bigrams.insert((chars[0], chars[1]), v);
        }
        let mut trigrams = HashMap::new();
        for (k, v) in trigrams_raw {
            let chars: Vec<char> = k.chars().collect();
            if chars.len() != 3 || chars.iter().any(|ch| cid(*ch).is_none()) {
                continue;
            }
            validate_count("trigram", &k, v)?;
            trigrams.insert((chars[0], chars[1], chars[2]), v);
        }
        let corpus = Self::new(letters, bigrams, trigrams);
        corpus.validate_totals()?;
        Ok(corpus)
    }

    fn count_text(text: &str) -> Self {
        let mut letters = HashMap::new();
        let mut bigrams = HashMap::new();
        let mut trigrams = HashMap::new();
        let mut run: Vec<char> = Vec::new();
        let mut flush = |run: &mut Vec<char>| {
            for ch in run.iter() {
                *letters.entry(*ch).or_insert(0.0) += 1.0;
            }
            for i in 0..run.len().saturating_sub(1) {
                *bigrams.entry((run[i], run[i + 1])).or_insert(0.0) += 1.0;
            }
            for i in 0..run.len().saturating_sub(2) {
                *trigrams
                    .entry((run[i], run[i + 1], run[i + 2]))
                    .or_insert(0.0) += 1.0;
            }
            run.clear();
        };
        for ch in text.chars() {
            if ch.is_ascii_lowercase() {
                run.push(ch);
            } else {
                flush(&mut run);
            }
        }
        flush(&mut run);
        Self::new(letters, bigrams, trigrams)
    }
}

fn single_char(s: &str) -> Option<char> {
    let mut it = s.chars();
    let ch = it.next()?;
    if it.next().is_some() {
        None
    } else {
        Some(ch)
    }
}

fn validate_count(kind: &str, key: &str, value: f64) -> Result<()> {
    if !value.is_finite() || value < 0.0 {
        return Err(anyhow!(
            "invalid {kind} count for {key:?}: expected a finite non-negative number, got {value}"
        ));
    }
    Ok(())
}

fn normalize_map<K>(map: &HashMap<K, f64>) -> HashMap<K, f64>
where
    K: Eq + std::hash::Hash + Copy,
{
    let total: f64 = map.values().sum();
    if total > 0.0 {
        map.iter().map(|(k, v)| (*k, *v / total)).collect()
    } else {
        HashMap::new()
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let data = read_bytes_limited(path, MAX_JSON_BYTES, "JSON file")?;
    serde_json::from_slice(&data).with_context(|| format!("invalid JSON in {}", path.display()))
}

pub(crate) fn parsed_dir(path: &Path) -> Option<PathBuf> {
    let path = expand_user(path);
    if path.is_file() {
        let name = path.file_name()?.to_string_lossy();
        if PARSED_FILES.contains(&name.as_ref()) {
            return path.parent().map(Path::to_path_buf);
        }
        return None;
    }
    if path.is_dir() {
        if PARSED_FILES.iter().all(|n| path.join(n).is_file()) {
            return Some(path);
        }
        let parsed = path.join("parsed");
        if PARSED_FILES.iter().all(|n| parsed.join(n).is_file()) {
            return Some(parsed);
        }
    }
    None
}

pub(crate) fn source_file(path: &Path) -> Result<Option<PathBuf>> {
    let path = expand_user(path);
    if path.is_file() {
        return Ok(Some(path));
    }
    if path.is_dir() {
        let mut candidates = Vec::new();
        for entry in std::fs::read_dir(&path)
            .with_context(|| format!("failed to read corpus directory {}", path.display()))?
        {
            let entry = entry
                .with_context(|| format!("failed to read corpus directory {}", path.display()))?;
            let path = entry.path();
            if path.is_file() && is_raw_source(&path) {
                candidates.push(path);
            }
        }
        candidates.sort();
        if candidates.len() > 1 {
            let names = candidates
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(anyhow!(
                "corpus directory {} contains multiple raw sources: {}; pass one file explicitly",
                path.display(),
                names
            ));
        }
        return Ok(candidates.into_iter().next());
    }
    Ok(None)
}

fn is_raw_source(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_lowercase().as_str(), "txt" | "xz" | "zip"))
        .unwrap_or(false)
}

pub(crate) fn expand_user(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(format!("{}/{}", home, rest));
        }
    }
    path.to_path_buf()
}

fn read_source(path: &Path) -> Result<String> {
    let suffix = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match suffix.as_str() {
        "xz" => {
            let input = read_bytes_limited(path, MAX_COMPRESSED_BYTES, "compressed corpus")?;
            let mut output = LimitedWriter::new(MAX_DECOMPRESSED_BYTES, "decompressed corpus");
            lzma_rs::xz_decompress(&mut &input[..], &mut output)
                .with_context(|| format!("failed to decompress {}", path.display()))?;
            decode_utf8(output.into_inner(), &path.display().to_string())
        }
        "zip" => {
            check_file_size(path, MAX_COMPRESSED_BYTES, "compressed corpus")?;
            let file =
                File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
            let mut archive = zip::ZipArchive::new(file)
                .with_context(|| format!("invalid zip {}", path.display()))?;
            let mut candidates: Vec<String> = (0..archive.len())
                .map(|i| archive.by_index(i).map(|f| f.name().to_string()))
                .collect::<std::result::Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|name| name.to_lowercase().ends_with(".txt") && !name.ends_with('/'))
                .collect();
            candidates.sort();
            let name = match candidates.as_slice() {
                [] => return Err(anyhow!("no .txt member in {}", path.display())),
                [name] => name.clone(),
                _ => {
                    return Err(anyhow!(
                        "{} contains multiple .txt members: {}",
                        path.display(),
                        candidates.join(", ")
                    ))
                }
            };
            let label = format!("{}:{}", path.display(), name);
            let member = archive.by_name(&name)?;
            if member.size() > MAX_DECOMPRESSED_BYTES {
                return Err(anyhow!(
                    "{} exceeds the {} MiB decompressed limit",
                    label,
                    MAX_DECOMPRESSED_BYTES / MIB
                ));
            }
            let mut output = Vec::new();
            member
                .take(MAX_DECOMPRESSED_BYTES + 1)
                .read_to_end(&mut output)
                .with_context(|| format!("failed to read {label}"))?;
            if output.len() as u64 > MAX_DECOMPRESSED_BYTES {
                return Err(anyhow!(
                    "{} exceeds the {} MiB decompressed limit",
                    label,
                    MAX_DECOMPRESSED_BYTES / MIB
                ));
            }
            decode_utf8(output, &label)
        }
        _ => {
            let bytes = read_bytes_limited(path, MAX_DECOMPRESSED_BYTES, "text file")?;
            decode_utf8(bytes, &path.display().to_string())
        }
    }
}

fn check_file_size(path: &Path, limit: u64, what: &str) -> Result<u64> {
    let size = std::fs::metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?
        .len();
    if size > limit {
        return Err(anyhow!(
            "{} {} exceeds the {} MiB limit: {} bytes",
            what,
            path.display(),
            limit / MIB,
            size
        ));
    }
    Ok(size)
}

fn read_bytes_limited(path: &Path, limit: u64, what: &str) -> Result<Vec<u8>> {
    check_file_size(path, limit, what)?;
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut data = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut data)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if data.len() as u64 > limit {
        return Err(anyhow!(
            "{} {} exceeds the {} MiB limit",
            what,
            path.display(),
            limit / MIB
        ));
    }
    Ok(data)
}

fn decode_utf8(bytes: Vec<u8>, label: &str) -> Result<String> {
    String::from_utf8(bytes)
        .map_err(|e| anyhow!("{} is not valid UTF-8: {}", label, e.utf8_error()))
}

struct LimitedWriter {
    data: Vec<u8>,
    limit: u64,
    label: &'static str,
}

impl LimitedWriter {
    fn new(limit: u64, label: &'static str) -> Self {
        Self {
            data: Vec::new(),
            limit,
            label,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.data
    }
}

impl Write for LimitedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.data.len() as u64 + buf.len() as u64 > self.limit {
            return Err(io::Error::other(format!(
                "{} exceeds the {} MiB limit",
                self.label,
                self.limit / MIB
            )));
        }
        self.data.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn load_many(paths: &[PathBuf]) -> Result<Corpus> {
    let corpora = paths
        .iter()
        .map(|p| Corpus::load(p))
        .collect::<Result<Vec<_>>>()?;
    Corpus::merge(&corpora)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir(name: &str) -> PathBuf {
        let seq = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "shirolyzer-corpus-{}-{}-{}",
            std::process::id(),
            name,
            seq
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_zip(path: &Path, members: &[(&str, &[u8])]) {
        let file = File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (name, data) in members {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }

    fn write_xz(path: &Path, data: &[u8]) {
        let mut output = Vec::new();
        lzma_rs::xz_compress(&mut std::io::Cursor::new(data), &mut output).unwrap();
        std::fs::write(path, output).unwrap();
    }

    #[test]
    fn rejects_ambiguous_source_directories_with_candidates() {
        let dir = temp_dir("ambiguous");
        std::fs::write(dir.join("a.txt"), b"alpha").unwrap();
        std::fs::write(dir.join("b.xz"), b"beta").unwrap();
        let err = Corpus::load(&dir).unwrap_err().to_string();
        assert!(err.contains("multiple raw sources"), "{err}");
        assert!(err.contains("a.txt"), "{err}");
        assert!(err.contains("b.xz"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_archives_with_multiple_text_members() {
        let dir = temp_dir("multi-member");
        let path = dir.join("multi.zip");
        write_zip(&path, &[("a.txt", b"alpha"), ("b.txt", b"beta")]);
        let err = Corpus::load(&path).unwrap_err().to_string();
        assert!(err.contains("multiple .txt members"), "{err}");
        assert!(err.contains("a.txt"), "{err}");
        assert!(err.contains("b.txt"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_invalid_utf8_in_raw_and_compressed_sources() {
        let dir = temp_dir("invalid-utf8");
        let raw = dir.join("bad.txt");
        std::fs::write(&raw, b"abc\xff").unwrap();
        let err = Corpus::load(&raw).unwrap_err().to_string();
        assert!(err.contains("not valid UTF-8"), "{err}");

        let zip = dir.join("bad.zip");
        write_zip(&zip, &[("bad.txt", b"abc\xff")]);
        let err = Corpus::load(&zip).unwrap_err().to_string();
        assert!(err.contains("not valid UTF-8"), "{err}");
        assert!(err.contains("bad.txt"), "{err}");

        let xz = dir.join("bad.xz");
        write_xz(&xz, b"abc\xff");
        let err = Corpus::load(&xz).unwrap_err().to_string();
        assert!(err.contains("not valid UTF-8"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_oversized_source_and_json_inputs() {
        let dir = temp_dir("oversized");

        let raw = dir.join("huge.txt");
        let file = File::create(&raw).unwrap();
        file.set_len(MAX_DECOMPRESSED_BYTES + 1).unwrap();
        drop(file);
        let err = Corpus::load(&raw).unwrap_err().to_string();
        assert!(err.contains("text file"), "{err}");
        assert!(err.contains("limit"), "{err}");

        let xz = dir.join("huge.xz");
        let file = File::create(&xz).unwrap();
        file.set_len(MAX_COMPRESSED_BYTES + 1).unwrap();
        drop(file);
        let err = Corpus::load(&xz).unwrap_err().to_string();
        assert!(err.contains("compressed corpus"), "{err}");
        assert!(err.contains("limit"), "{err}");

        let zip = dir.join("huge.zip");
        let file = File::create(&zip).unwrap();
        file.set_len(MAX_COMPRESSED_BYTES + 1).unwrap();
        drop(file);
        let err = Corpus::load(&zip).unwrap_err().to_string();
        assert!(err.contains("compressed corpus"), "{err}");
        assert!(err.contains("limit"), "{err}");

        let parsed = dir.join("parsed");
        std::fs::create_dir_all(&parsed).unwrap();
        std::fs::write(parsed.join(PARSED_FILES[0]), b"{}").unwrap();
        std::fs::write(parsed.join(PARSED_FILES[1]), b"{}").unwrap();
        let trigrams = File::create(parsed.join(PARSED_FILES[2])).unwrap();
        trigrams.set_len(MAX_JSON_BYTES + 1).unwrap();
        drop(trigrams);
        let err = Corpus::load(&parsed).unwrap_err().to_string();
        assert!(err.contains("JSON file"), "{err}");
        assert!(err.contains("limit"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn byte_and_decompression_limits_are_enforced() {
        let dir = temp_dir("limits");
        let path = dir.join("data.txt");
        std::fs::write(&path, b"hello").unwrap();
        let err = read_bytes_limited(&path, 4, "test file")
            .unwrap_err()
            .to_string();
        assert!(err.contains("test file"), "{err}");
        assert!(err.contains("limit"), "{err}");

        let mut writer = LimitedWriter::new(4, "decompressed test");
        assert_eq!(writer.write(b"four").unwrap(), 4);
        assert!(writer.write(b"more").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    fn trigram_corpus(ngram: (char, char, char), count: f64) -> Corpus {
        Corpus::new(
            HashMap::from([('a', 1.0), ('b', 1.0), ('c', 1.0)]),
            HashMap::new(),
            HashMap::from([(ngram, count)]),
        )
    }

    #[test]
    fn recomputes_totals_after_filtering_unsupported_keys() {
        let corpus = Corpus::new(
            HashMap::from([('a', 2.0), ('!', 100.0)]),
            HashMap::from([(('a', 'b'), 3.0), (('a', '!'), 100.0)]),
            HashMap::from([(('a', 'b', 'c'), 4.0), (('a', '!', 'b'), 100.0)]),
        );
        assert_eq!(corpus.total_letters, 2.0);
        assert_eq!(corpus.total_bigrams, 3.0);
        assert_eq!(corpus.total_trigrams, 4.0);
        assert_eq!(corpus.bi_ids.len(), 1);
        assert_eq!(corpus.tri_ids.len(), 1);
        assert!(!corpus.letters.contains_key(&'!'));
        assert!(!corpus.bigrams.contains_key(&('a', '!')));
        assert!(!corpus.trigrams.contains_key(&('a', '!', 'b')));
    }

    #[test]
    fn rejects_invalid_parsed_counts() {
        let mut letters = HashMap::new();
        letters.insert("a".to_string(), f64::NAN);
        let err = Corpus::from_raw_maps(letters, HashMap::new(), HashMap::new())
            .unwrap_err()
            .to_string();
        assert!(err.contains("letter"), "{err}");
        assert!(err.contains("finite non-negative"), "{err}");

        let mut bigrams = HashMap::new();
        bigrams.insert("ab".to_string(), -1.0);
        let err = Corpus::from_raw_maps(HashMap::new(), bigrams, HashMap::new())
            .unwrap_err()
            .to_string();
        assert!(err.contains("bigram"), "{err}");
        assert!(err.contains("finite non-negative"), "{err}");

        let mut trigrams = HashMap::new();
        trigrams.insert("abc".to_string(), f64::INFINITY);
        let err = Corpus::from_raw_maps(HashMap::new(), HashMap::new(), trigrams)
            .unwrap_err()
            .to_string();
        assert!(err.contains("trigram"), "{err}");
        assert!(err.contains("finite non-negative"), "{err}");
    }

    #[test]
    fn parsed_loader_filters_invalid_keys_and_validates_counts() {
        let dir = temp_dir("parsed-counts");
        let parsed = dir.join("parsed");
        std::fs::create_dir_all(&parsed).unwrap();
        std::fs::write(parsed.join(PARSED_FILES[0]), r#"{"a": 1.0, "!": 99.0}"#).unwrap();
        std::fs::write(parsed.join(PARSED_FILES[1]), r#"{"ab": 2.0, "a!": 99.0}"#).unwrap();
        std::fs::write(parsed.join(PARSED_FILES[2]), r#"{"abc": 3.0, "a!b": 99.0}"#).unwrap();

        let corpus = Corpus::load(&parsed).unwrap();
        assert_eq!(corpus.total_letters, 1.0);
        assert_eq!(corpus.total_bigrams, 2.0);
        assert_eq!(corpus.total_trigrams, 3.0);
        assert_eq!(corpus.bi_ids.len(), 1);
        assert_eq!(corpus.tri_ids.len(), 1);

        std::fs::write(parsed.join(PARSED_FILES[2]), r#"{"abc": -1.0}"#).unwrap();
        let err = format!("{:#}", Corpus::load(&parsed).unwrap_err());
        assert!(err.contains("invalid corpus counts"), "{err}");
        assert!(err.contains("trigram"), "{err}");
        assert!(err.contains("finite non-negative"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn parsed_loader_rejects_missing_lower_order_counts() {
        let dir = temp_dir("parsed-mismatch");
        let parsed = dir.join("parsed");
        std::fs::create_dir_all(&parsed).unwrap();
        std::fs::write(parsed.join(PARSED_FILES[0]), "{}").unwrap();
        std::fs::write(parsed.join(PARSED_FILES[1]), "{}").unwrap();
        std::fs::write(parsed.join(PARSED_FILES[2]), r#"{"abc": 1.0}"#).unwrap();
        let err = Corpus::load(&parsed).unwrap_err().to_string();
        assert!(err.contains("no supported letter counts"), "{err}");

        std::fs::write(parsed.join(PARSED_FILES[0]), r#"{"a": 1.0}"#).unwrap();
        let err = Corpus::load(&parsed).unwrap_err().to_string();
        assert!(err.contains("trigrams but no bigram counts"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn raw_text_breaks_runs_at_spaces_and_punctuation() {
        let dir = temp_dir("raw-tokenization");
        let path = dir.join("words.txt");
        std::fs::write(&path, "Ab-cd ef.\n").unwrap();
        let corpus = Corpus::load(&path).unwrap();
        assert_eq!(corpus.total_letters, 6.0);
        assert_eq!(corpus.total_bigrams, 3.0);
        assert_eq!(corpus.total_trigrams, 0.0);
        for pair in [('a', 'b'), ('c', 'd'), ('e', 'f')] {
            assert_eq!(corpus.bigrams.get(&pair), Some(&1.0), "{pair:?}");
        }
        assert!(!corpus.bigrams.contains_key(&('b', 'c')));
        assert!(!corpus.bigrams.contains_key(&('d', 'e')));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn normalized_equal_weights_blend_by_distribution() {
        let large = trigram_corpus(('a', 'b', 'c'), 100.0);
        let small = trigram_corpus(('a', 'b', 'd'), 1.0);

        let raw = Corpus::merge(&[large.clone(), small.clone()]).unwrap();
        assert_eq!(raw.trigrams[&('a', 'b', 'c')], 100.0);
        assert_eq!(raw.trigrams[&('a', 'b', 'd')], 1.0);

        let normalized = Corpus::merge(&[large.normalized(), small.normalized()]).unwrap();
        assert!((normalized.trigrams[&('a', 'b', 'c')] - 1.0).abs() < 1e-12);
        assert!((normalized.trigrams[&('a', 'b', 'd')] - 1.0).abs() < 1e-12);
        assert!((normalized.total_trigrams - 2.0).abs() < 1e-12);
    }

    #[test]
    fn scaling_rejects_invalid_factors() {
        let corpus = trigram_corpus(('a', 'b', 'c'), 1.0);
        assert!(corpus.scaled(f64::NAN).is_err());
        assert!(corpus.scaled(-1.0).is_err());
        assert!((corpus.scaled(2.0).unwrap().total_trigrams - 2.0).abs() < 1e-12);
    }
}
