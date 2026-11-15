//! Same-finger skipgram (SFS) tier preferences.
//!
//! Patterns are expressed as per-hand finger ranks, where `1 = index`,
//! `2 = middle`, `3 = ring`, and `4 = pinky` (the same notation as
//! `redirect_tiers`).  An SFS is a triple whose first and third physical
//! fingers match; the middle finger differs or is the same finger type on the
//! other hand, for example `1-2-1` or `1-1-1`.  The repeated outer finger fixes
//! the hand; flexible fingering may put the middle key on the other hand, and
//! its rank still describes the finger type.
//!
//! Tiers run from most preferred (tier 1) to least preferred.  Weights are
//! multipliers on the distance-dependent base SFS cost, so the existing
//! near/far distinction is preserved.  Unlisted patterns keep a multiplier of
//! `1.0`.

use crate::redirect::rank;

/// Whether a rank triple is a same-finger skipgram pattern.
///
/// `1-1-1` is accepted because it represents a cross-hand middle of the same
/// finger type; `pattern_for` never returns it for three presses of one
/// physical finger.
#[inline]
pub fn is_sfs(pattern: [u8; 3]) -> bool {
    pattern[0] == pattern[2]
}

/// Classify three eight-state fingers as an SFS pattern.
///
/// The outer finger fixes the hand.  The middle finger may be an alternate
/// cross-hand finger; its rank still describes which finger type it is.
pub fn pattern_for(fs: [usize; 3]) -> Option<[u8; 3]> {
    if fs[0] != fs[2] || fs[0] == fs[1] {
        return None;
    }
    Some([rank(fs[0]), rank(fs[1]), rank(fs[2])])
}

/// Render a configured pattern the way it appears in the YAML file.
pub fn pattern_label(pattern: [u8; 3]) -> String {
    format!("{}-{}-{}", pattern[0], pattern[1], pattern[2])
}

/// Flat key for per-pattern analyzer output.
pub fn pattern_key(pattern: [u8; 3]) -> String {
    format!("sfs_{}_{}_{}", pattern[0], pattern[1], pattern[2])
}

/// Default multiplier for a zero-based tier in a table with `tier_count` tiers.
/// The best tier is neutral (`1.0`) and the worst tier is `2.0`.
pub fn tier_multiplier(tier: usize, tier_count: usize) -> f64 {
    let span = tier_count.saturating_sub(1).max(1) as f64;
    1.0 + tier.min(tier_count.saturating_sub(1)) as f64 / span
}

/// Ordered SFS tiers, most preferred first.
#[derive(Clone, Debug, Default)]
pub struct SfsTiers {
    tiers: Vec<Vec<[u8; 3]>>,
    weights: Option<Vec<f64>>,
}

impl SfsTiers {
    pub fn new(tiers: Vec<Vec<[u8; 3]>>) -> Self {
        Self {
            tiers,
            weights: None,
        }
    }

    pub fn with_weights(tiers: Vec<Vec<[u8; 3]>>, weights: Vec<f64>) -> Self {
        Self {
            tiers,
            weights: Some(weights),
        }
    }

    pub fn tiers(&self) -> &[Vec<[u8; 3]>] {
        &self.tiers
    }

    pub fn weights(&self) -> Option<&[f64]> {
        self.weights.as_deref()
    }

    pub fn has_weights(&self) -> bool {
        self.weights.is_some()
    }

    pub fn len(&self) -> usize {
        self.tiers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiers.is_empty()
    }

    /// Zero-based tier index for a configured pattern.
    pub fn tier_of(&self, pattern: [u8; 3]) -> Option<usize> {
        self.tiers.iter().position(|tier| tier.contains(&pattern))
    }

    pub fn base_multiplier(&self, tier: usize) -> f64 {
        self.weights
            .as_ref()
            .and_then(|weights| weights.get(tier).copied())
            .unwrap_or_else(|| tier_multiplier(tier, self.tiers.len()))
    }

    pub fn multiplier(&self, pattern: [u8; 3]) -> Option<f64> {
        self.tier_of(pattern).map(|tier| self.base_multiplier(tier))
    }

    pub fn table(&self) -> SfsTierTable {
        let mut multipliers = [[[None; 4]; 4]; 4];
        for (tier, patterns) in self.tiers.iter().enumerate() {
            let multiplier = self.base_multiplier(tier);
            for &pattern in patterns {
                multipliers[(pattern[0] - 1) as usize][(pattern[1] - 1) as usize]
                    [(pattern[2] - 1) as usize] = Some(multiplier);
            }
        }
        SfsTierTable { multipliers }
    }
}

/// Copyable lookup table installed in [`crate::model::Scales`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SfsTierTable {
    multipliers: [[[Option<f64>; 4]; 4]; 4],
}

impl SfsTierTable {
    #[inline]
    pub fn multiplier(&self, pattern: [u8; 3]) -> Option<f64> {
        self.multipliers[(pattern[0] - 1) as usize][(pattern[1] - 1) as usize]
            [(pattern[2] - 1) as usize]
    }

    #[inline]
    pub fn multiplier_for_fingers(&self, fs: [usize; 3]) -> Option<f64> {
        pattern_for(fs).and_then(|pattern| self.multiplier(pattern))
    }
}

/// Multiplier for a same-finger skipgram, or `1.0` when it is unlisted or no
/// table is configured.
#[inline]
pub fn multiplier(table: Option<&SfsTierTable>, fs: [usize; 3]) -> f64 {
    table
        .and_then(|table| table.multiplier_for_fingers(fs))
        .unwrap_or(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::parse_finger8;

    fn fingers(pattern: [u8; 3], hand: usize) -> [usize; 3] {
        pattern.map(|rank| hand * 4 + (4 - rank) as usize)
    }

    #[test]
    fn patterns_require_equal_outer_fingers() {
        assert_eq!(pattern_for(fingers([1, 2, 1], 0)), Some([1, 2, 1]));
        assert_eq!(pattern_for(fingers([4, 3, 4], 1)), Some([4, 3, 4]));
        assert_eq!(pattern_for(fingers([1, 2, 3], 0)), None);
        assert_eq!(pattern_for(fingers([1, 1, 1], 0)), None);
        assert_eq!(
            pattern_for([
                parse_finger8("LI").unwrap(),
                parse_finger8("RM").unwrap(),
                parse_finger8("LI").unwrap(),
            ]),
            Some([1, 2, 1])
        );
        assert_eq!(
            pattern_for([
                parse_finger8("LI").unwrap(),
                parse_finger8("RI").unwrap(),
                parse_finger8("LI").unwrap(),
            ]),
            Some([1, 1, 1])
        );
        assert_eq!(
            pattern_for([
                parse_finger8("LI").unwrap(),
                parse_finger8("RM").unwrap(),
                parse_finger8("RI").unwrap(),
            ]),
            None
        );
    }

    #[test]
    fn multipliers_scale_by_tier_and_leave_unlisted_patterns_neutral() {
        let tiers =
            SfsTiers::with_weights(vec![vec![[1, 2, 1]], vec![[4, 3, 4]]], vec![0.75, 1.75]);
        assert_eq!(tiers.multiplier([1, 2, 1]), Some(0.75));
        assert_eq!(tiers.multiplier([4, 3, 4]), Some(1.75));
        assert_eq!(tiers.multiplier([2, 1, 2]), None);

        let table = tiers.table();
        let left = fingers([1, 2, 1], 0);
        let right = fingers([1, 2, 1], 1);
        assert_eq!(table.multiplier_for_fingers(left), Some(0.75));
        assert_eq!(table.multiplier_for_fingers(right), Some(0.75));
        assert_eq!(multiplier(Some(&table), left), 0.75);
        assert_eq!(multiplier(Some(&table), fingers([2, 1, 2], 0)), 1.0);
        assert_eq!(multiplier(None, left), 1.0);
    }

    #[test]
    fn default_multipliers_span_neutral_to_double() {
        let tiers = SfsTiers::new(vec![vec![[1, 2, 1]], vec![[2, 1, 2]], vec![[4, 3, 4]]]);
        assert_eq!(tiers.base_multiplier(0), 1.0);
        assert_eq!(tiers.base_multiplier(2), 2.0);
        let one = SfsTiers::new(vec![vec![[1, 2, 1]]]);
        assert_eq!(one.base_multiplier(0), 1.0);
    }

    #[test]
    fn pattern_keys_match_analyzer_notation() {
        assert_eq!(pattern_key([1, 2, 1]), "sfs_1_2_1");
        assert_eq!(pattern_label([4, 3, 4]), "4-3-4");
        assert!(is_sfs([1, 2, 1]));
        assert!(is_sfs([1, 1, 1]));
        assert!(!is_sfs([1, 2, 3]));
    }
}
