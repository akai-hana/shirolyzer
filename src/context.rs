//! Shared command context and corpus resolution.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::conditions::Conditions;
use crate::config::{self, Config, CorpusEntry};
use crate::corpus::Corpus;
use crate::gates::Gates;
use crate::geometry::Geometry;
use crate::model::Model;
use crate::paths;
use crate::scorer::Tables;

pub struct Context {
    pub cfg: Config,
    pub geom: Geometry,
    pub model: Model,
    pub gates: Gates,
    pub conditions: Conditions,
    pub tables: Tables,
}

impl Context {
    pub fn load(config_path: Option<&Path>) -> Result<Self> {
        let cfg = config::load(config_path)?;
        let geom = Geometry::from_file(&cfg.geometry)?;
        let model = config::build_model(&cfg)?;
        let gates = Gates::from_config(&cfg);
        let conditions = Conditions::from_config(&cfg, &geom);
        let tables = Tables::build(&model, &geom, &cfg, &gates, &conditions)?;
        Ok(Self {
            cfg,
            geom,
            model,
            gates,
            conditions,
            tables,
        })
    }

    /// Model context with the calibrated (unscaled) objective weights.  The
    /// audit checks the fitted model itself, independently of the user's
    /// priority list.
    pub fn calibration(&self) -> Result<Self> {
        let cfg = self.cfg.clone();
        let geom = self.geom.clone();
        let mut model = config::build_model(&cfg)?;
        model.scales = crate::model::Scales::default();
        let gates = Gates::from_config(&cfg);
        let conditions = Conditions::from_config(&cfg, &geom);
        let tables = Tables::build(&model, &geom, &cfg, &gates, &conditions)?;
        Ok(Self {
            cfg,
            geom,
            model,
            gates,
            conditions,
            tables,
        })
    }
}

fn expand_user(value: &str) -> PathBuf {
    if let Some(rest) = value.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(value)
}

pub fn corpus_path(_cfg: &Config, entry: &CorpusEntry) -> PathBuf {
    let value = entry.label();
    let path = expand_user(&value);
    if path.is_file() || path.is_dir() {
        return path;
    }
    let candidate = paths::corpus_root().join(&value).join("parsed");
    if candidate.is_dir() {
        return candidate;
    }
    path
}

pub fn corpus_labels(cfg: &Config, role: &str) -> Vec<String> {
    let entries = if role == "primary" {
        &cfg.primary_corpus
    } else {
        &cfg.secondary_corpus
    };
    entries.iter().map(CorpusEntry::label).collect()
}

pub fn load_role(cfg: &Config, role: &str) -> Result<(Corpus, Vec<String>)> {
    let entries = if role == "primary" {
        &cfg.primary_corpus
    } else {
        &cfg.secondary_corpus
    };
    if entries.is_empty() {
        return Err(anyhow!("config has no {} corpora", role));
    }
    let mut corpora = Vec::new();
    let mut labels = Vec::new();
    for entry in entries {
        let path = corpus_path(cfg, entry);
        labels.push(entry.label());
        let corpus = Corpus::load(&path)?;
        let corpus = if entry.normalize {
            corpus.normalized()
        } else {
            corpus
        };
        corpora.push(corpus.scaled(entry.weight)?);
    }
    Ok((Corpus::merge(&corpora)?, labels))
}

pub fn load_paths(paths_: &[String]) -> Result<Corpus> {
    let paths: Vec<PathBuf> = paths_.iter().map(|p| expand_user(p)).collect();
    crate::corpus::load_many(&paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir(name: &str) -> PathBuf {
        let seq = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "shirolyzer-context-{}-{}-{}",
            std::process::id(),
            name,
            seq
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_parsed_corpus(dir: &Path, scale: f64) {
        let parsed = dir.join("parsed");
        std::fs::create_dir_all(&parsed).unwrap();
        std::fs::write(
            parsed.join(crate::corpus::PARSED_FILES[0]),
            format!(r#"{{"a": {scale}}}"#),
        )
        .unwrap();
        std::fs::write(
            parsed.join(crate::corpus::PARSED_FILES[1]),
            format!(r#"{{"ab": {scale}}}"#),
        )
        .unwrap();
        std::fs::write(
            parsed.join(crate::corpus::PARSED_FILES[2]),
            format!(r#"{{"abc": {scale}}}"#),
        )
        .unwrap();
    }

    #[test]
    fn load_role_normalizes_entries_before_applying_weights() {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/config.reference.yaml");
        let mut cfg = config::load(Some(&fixture)).unwrap();
        let large = temp_dir("normalize-large");
        let small = temp_dir("normalize-small");
        write_parsed_corpus(&large, 100.0);
        write_parsed_corpus(&small, 1.0);
        cfg.primary_corpus = vec![
            CorpusEntry {
                name: None,
                path: Some(large.to_string_lossy().into_owned()),
                weight: 1.0,
                normalize: true,
            },
            CorpusEntry {
                name: None,
                path: Some(small.to_string_lossy().into_owned()),
                weight: 1.0,
                normalize: true,
            },
        ];

        let (corpus, _) = load_role(&cfg, "primary").unwrap();
        assert!((corpus.total_letters - 2.0).abs() < 1e-12);
        assert!((corpus.total_bigrams - 2.0).abs() < 1e-12);
        assert!((corpus.total_trigrams - 2.0).abs() < 1e-12);

        for entry in &mut cfg.primary_corpus {
            entry.normalize = false;
        }
        let (raw, _) = load_role(&cfg, "primary").unwrap();
        assert_eq!(raw.total_letters, 101.0);
        assert_eq!(raw.total_bigrams, 101.0);
        assert_eq!(raw.total_trigrams, 101.0);

        let _ = std::fs::remove_dir_all(large);
        let _ = std::fs::remove_dir_all(small);
    }
}
