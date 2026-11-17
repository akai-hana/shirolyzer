//! Shared validation for the 30-key layout strings accepted by CLI commands.

pub const LAYOUT_LENGTH: usize = 30;
pub const ROW_LENGTH: usize = 10;
pub const LETTER_COUNT: usize = 26;
pub const NON_LETTER_COUNT: usize = LAYOUT_LENGTH - LETTER_COUNT;

/// Check that `layout` contains 26 distinct ASCII lowercase letters followed
/// by four distinct printable non-letter characters.  Positions in errors are
/// one-based so they match what users see in a terminal.
pub fn validate_layout(layout: &str) -> Result<(), String> {
    let chars: Vec<char> = layout.chars().collect();
    if chars.len() != LAYOUT_LENGTH {
        return Err(format!(
            "layout must be {} characters, got {}",
            LAYOUT_LENGTH,
            chars.len()
        ));
    }

    let mut letters = [false; LETTER_COUNT];
    let mut non_letters = Vec::with_capacity(NON_LETTER_COUNT);
    for (pos, &ch) in chars.iter().enumerate() {
        if ch.is_ascii_lowercase() {
            let letter = (ch as u8 - b'a') as usize;
            if letters[letter] {
                return Err(format!(
                    "layout repeats letter {ch:?} at position {}",
                    pos + 1
                ));
            }
            letters[letter] = true;
        } else if ch.is_whitespace() {
            return Err(format!(
                "layout contains whitespace {ch:?} at position {}",
                pos + 1
            ));
        } else if !ch.is_ascii() {
            return Err(format!(
                "layout contains non-ASCII character {ch:?} at position {}",
                pos + 1
            ));
        } else if !ch.is_ascii_punctuation() {
            return Err(format!(
                "layout contains unsupported character {ch:?} at position {}",
                pos + 1
            ));
        } else if non_letters.contains(&ch) {
            return Err(format!(
                "layout repeats character {ch:?} at position {}",
                pos + 1
            ));
        } else {
            non_letters.push(ch);
        }
    }

    let missing: String = ('a'..='z')
        .filter(|&ch| !letters[(ch as u8 - b'a') as usize])
        .collect();
    if !missing.is_empty() {
        return Err(format!("layout is missing letters: {missing}"));
    }
    if non_letters.len() != NON_LETTER_COUNT {
        return Err(format!(
            "layout must contain {} non-letter characters, got {}",
            NON_LETTER_COUNT,
            non_letters.len()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = "abcdefghijklmnopqrstuvwxyz',./";

    #[test]
    fn accepts_a_complete_layout() {
        assert!(validate_layout(VALID).is_ok());
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(validate_layout("abc")
            .unwrap_err()
            .contains("must be 30 characters, got 3"));
    }

    #[test]
    fn rejects_duplicate_letters() {
        let err = validate_layout("aacdefghijklmnopqrstuvwxyz',./").unwrap_err();
        assert!(err.contains("repeats letter 'a'"), "{err}");
    }

    #[test]
    fn rejects_missing_letters() {
        let err = validate_layout("acdefghijklmnopqrstuvwxyz',./;").unwrap_err();
        assert!(err.contains("missing letters: b"), "{err}");
    }

    #[test]
    fn rejects_uppercase_letters() {
        let err = validate_layout("Abcdefghijklmnopqrstuvwxyz',./").unwrap_err();
        assert!(err.contains("unsupported character 'A'"), "{err}");
    }

    #[test]
    fn rejects_punctuation_only_layouts() {
        let layout: String = "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~"
            .chars()
            .take(LAYOUT_LENGTH)
            .collect();
        let err = validate_layout(&layout).unwrap_err();
        assert!(err.contains("missing letters"), "{err}");
    }

    #[test]
    fn rejects_multibyte_punctuation_without_panicking() {
        let err = validate_layout("ébcdefghijklmnopqrstuvwxyz',./").unwrap_err();
        assert!(err.contains("non-ASCII character 'é'"), "{err}");
    }

    #[test]
    fn rejects_whitespace() {
        let err = validate_layout("abcdefghijklmnopqrstuvwxyz ,./").unwrap_err();
        assert!(err.contains("whitespace ' '"), "{err}");
    }

    #[test]
    fn rejects_duplicate_punctuation() {
        let err = validate_layout("abcdefghijklmnopqrstuvwxyz',,.").unwrap_err();
        assert!(err.contains("repeats character ','"), "{err}");
    }

    #[test]
    fn rejects_digits() {
        let err = validate_layout("abcdefghijklmnopqrstuvwxyz1,./").unwrap_err();
        assert!(err.contains("unsupported character '1'"), "{err}");
    }
}
