//! Forbidden-condition evaluator used when `conditions` has a non-default set.
//!
//! Port of `utils/conditions.py`.

use crate::config::{Condition, Config};
use crate::geometry::{finger_hand, finger_kind, Geometry, NF, NK};
use crate::model::{pair_feature, pt_idx};

pub const PENALTY: f64 = 12.0;

#[derive(Clone, Debug)]
pub struct Conditions {
    pub enabled: bool,
    pub rules: Vec<Condition>,
    pub pair_denied: Option<Vec<bool>>,
}

impl Conditions {
    pub fn from_config(cfg: &Config, geom: &Geometry) -> Self {
        let enabled = !cfg.conditions_are_default;
        let rules = if enabled {
            cfg.conditions.clone()
        } else {
            Vec::new()
        };
        let mut pair_denied = None;
        if enabled {
            let mut denied = vec![false; NK * NF * NK * NF];
            for a in 0..NK {
                for fa in 0..NF {
                    for b in 0..NK {
                        for fb in 0..NF {
                            denied[pt_idx(a, fa, b, fb)] =
                                !allowed_pair(geom, &rules, a, fa, b, fb);
                        }
                    }
                }
            }
            pair_denied = Some(denied);
        }
        Self {
            enabled,
            rules,
            pair_denied,
        }
    }

    pub fn matches_fingers(&self, cond: &Condition, fa: usize, fb: usize) -> bool {
        let Some(fingers) = &cond.fingers else {
            return true;
        };
        let a = (finger_hand(fa), finger_kind(fa));
        let b = (finger_hand(fb), finger_kind(fb));
        fingers.contains(&a) || fingers.contains(&b)
    }

    pub fn matches_pairs(&self, cond: &Condition, fa: usize, fb: usize) -> bool {
        let Some(pairs) = &cond.pairs else {
            return true;
        };
        let ab = (
            finger_hand(fa),
            finger_kind(fa),
            finger_hand(fb),
            finger_kind(fb),
        );
        pairs
            .iter()
            .any(|[x, y]| ab == (x.0, x.1, y.0, y.1) || ab == (y.0, y.1, x.0, x.1))
    }

    pub fn condition_matches(
        &self,
        cond: &Condition,
        fa: usize,
        fb: usize,
        pf: &crate::model::PairFeature,
    ) -> bool {
        if !self.matches_fingers(cond, fa, fb) || !self.matches_pairs(cond, fa, fb) {
            return false;
        }
        match cond.kind.as_str() {
            "scissors" => {
                let drow = pf.drow;
                let matched = match cond.distance.as_deref() {
                    Some("full") => drow.abs() >= 2.0,
                    Some("half") => drow.abs() == 1.0,
                    _ => false,
                };
                matched && (!cond.reverse || pf.g < 0.0)
            }
            "spread" => pf.nadj == 1 && pf.s > pf.rest_sep + cond.max_extra,
            "separation" => pf.s < cond.min_width,
            "crossing" => pf.nadj >= 2 && pf.s < cond.deficit + 1.0,
            _ => false,
        }
    }

    pub fn penalty_mask(&self, keys: &[char]) -> Vec<f64> {
        let n = keys.len();
        let mut size = 1usize;
        for _ in 0..n {
            size *= NF;
        }
        let mut out = vec![0.0; size];
        self.penalty_mask_into(keys, &mut out);
        out
    }

    pub fn penalty_mask_into(&self, keys: &[char], out: &mut [f64]) {
        let n = keys.len();
        let mut size = 1usize;
        for _ in 0..n {
            size *= NF;
        }
        let out = &mut out[..size];
        out.fill(0.0);
        if !self.enabled || n < 2 {
            return;
        }
        let Some(denied) = &self.pair_denied else {
            return;
        };
        for (idx, value) in out.iter_mut().enumerate() {
            let mut penalty = 0.0;
            let mut div = 1usize;
            let mut fingers = [0usize; 3];
            for pos in (0..n).rev() {
                fingers[pos] = (idx / div) % NF;
                div *= NF;
            }
            for pos in 0..n - 1 {
                let a = crate::geometry::key_index_char(keys[pos]).unwrap();
                let b = crate::geometry::key_index_char(keys[pos + 1]).unwrap();
                if denied[pt_idx(a, fingers[pos], b, fingers[pos + 1])] {
                    penalty += PENALTY;
                }
            }
            *value = penalty;
        }
    }
}

pub fn allowed_pair(
    geom: &Geometry,
    rules: &[Condition],
    a: usize,
    fa: usize,
    b: usize,
    fb: usize,
) -> bool {
    if finger_hand(fa) != finger_hand(fb) || finger_kind(fa) == finger_kind(fb) {
        return true;
    }
    let pf = pair_feature(geom, a, fa, b, fb);
    let conditions = Conditions {
        enabled: true,
        rules: rules.to_vec(),
        pair_denied: None,
    };
    for cond in rules {
        if conditions.condition_matches(cond, fa, fb, &pf) {
            return cond.allow;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::parse_finger8;

    fn condition_rules() -> Vec<(&'static str, Condition)> {
        let base = Condition {
            kind: String::new(),
            allow: false,
            distance: None,
            reverse: false,
            fingers: None,
            pairs: None,
            max_extra: 2.0,
            min_width: 1.9,
            deficit: 0.30,
        };
        vec![
            (
                "scissors",
                Condition {
                    kind: "scissors".into(),
                    distance: Some("full".into()),
                    ..base.clone()
                },
            ),
            (
                "spread",
                Condition {
                    kind: "spread".into(),
                    max_extra: 0.0,
                    ..base.clone()
                },
            ),
            (
                "separation",
                Condition {
                    kind: "separation".into(),
                    min_width: 1e9,
                    ..base.clone()
                },
            ),
            (
                "crossing",
                Condition {
                    kind: "crossing".into(),
                    deficit: 1e9,
                    ..base
                },
            ),
        ]
    }

    fn find_same_hand_match(
        geom: &Geometry,
        rule: &Condition,
    ) -> Option<(usize, usize, usize, usize)> {
        let conditions = Conditions {
            enabled: true,
            rules: vec![rule.clone()],
            pair_denied: None,
        };
        for a in 0..NK {
            for fa in 0..NF {
                if finger_hand(fa) != 0 {
                    continue;
                }
                for b in 0..NK {
                    for fb in 0..NF {
                        if finger_hand(fb) != 0 || finger_kind(fa) == finger_kind(fb) {
                            continue;
                        }
                        let pf = pair_feature(geom, a, fa, b, fb);
                        if conditions.condition_matches(rule, fa, fb, &pf) {
                            return Some((a, fa, b, fb));
                        }
                    }
                }
            }
        }
        None
    }

    #[test]
    fn same_hand_condition_rules_deny_matching_pairs() {
        let geom = Geometry::new();
        for (name, rule) in condition_rules() {
            let (a, fa, b, fb) = find_same_hand_match(&geom, &rule)
                .unwrap_or_else(|| panic!("no same-hand pair matches {name}"));
            assert!(
                !allowed_pair(&geom, &[rule], a, fa, b, fb),
                "{name} did not deny a matching same-hand pair"
            );
        }
    }

    #[test]
    fn cross_hand_pairs_bypass_condition_rules() {
        let geom = Geometry::new();
        let a = crate::geometry::key_index("Q").unwrap();
        let b = crate::geometry::key_index("P").unwrap();
        let li = parse_finger8("LI").unwrap();
        let ri = parse_finger8("RI").unwrap();
        for (name, rule) in condition_rules() {
            assert!(
                allowed_pair(&geom, &[rule], a, li, b, ri),
                "{name} restricted a cross-hand pair"
            );
        }
    }
}
