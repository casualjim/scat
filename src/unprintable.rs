//! Unprintable character display functionality.
//! Similar to `cat -A` or `bat -A`, but maintains syntax highlighting.

/// Character display style for unprintable characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharStyle {
    /// Use Unicode symbols (→, ↵, ·, ␊, etc.)
    Unicode,
    /// Use caret notation (^I, ^M, etc.)
    Caret,
}

/// Detect if terminal supports UTF-8 by checking locale env vars.
fn detect_utf8() -> bool {
    std::env::var("LANG")
        .or_else(|_| std::env::var("LC_CTYPE"))
        .or_else(|_| std::env::var("LC_ALL"))
        .map(|v| v.to_ascii_lowercase().contains("utf"))
        .unwrap_or(true) // Default to UTF-8 for modern systems
}

/// Get the appropriate character style based on terminal capabilities.
pub fn get_char_style() -> CharStyle {
    if detect_utf8() {
        CharStyle::Unicode
    } else {
        CharStyle::Caret
    }
}

/// Transform unprintable characters to their visual representations.
///
/// This transforms spaces, tabs, newlines, and other control characters
/// into visible symbols for display.
///
/// # Arguments
/// * `text` - The text to transform
/// * `style` - The character style to use (Unicode or Caret)
///
/// # Returns
/// A new string with unprintable characters replaced by their visual representations
pub fn show_unprintable(text: &str, style: CharStyle) -> String {
    let mut result = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        match c {
            ' ' => result.push('·'),
            '\t' => {
                if matches!(style, CharStyle::Unicode) {
                    result.push('→');
                } else {
                    result.push_str("^I");
                }
            }
            '\n' => {
                if matches!(style, CharStyle::Unicode) {
                    result.push_str("␊\n");
                } else {
                    result.push_str("$\n");
                }
            }
            '\r' => {
                if matches!(style, CharStyle::Unicode) {
                    result.push('↵');
                } else {
                    result.push_str("^M");
                }
            }
            '\u{1b}' => {
                if matches!(style, CharStyle::Unicode) {
                    result.push('␛');
                } else {
                    result.push_str("^[");
                }
            }
            '\0' => {
                if matches!(style, CharStyle::Unicode) {
                    result.push('␀');
                } else {
                    result.push_str("^@");
                }
            }
            c if c.is_control() && (c as u32) <= 0x1F => {
                // Control characters 0x01-0x1F
                if matches!(style, CharStyle::Unicode) {
                    // Use Unicode control picture symbols (U+2400 range)
                    // ␀ is U+2400, so we add 0x2400 to the control code
                    if let Some(ch) = char::from_u32(c as u32 + 0x2400) {
                        result.push(ch);
                    }
                } else {
                    result.push('^');
                    result.push((c as u8 + b'@') as char);
                }
            }
            '\x7f' => {
                // DEL character (0x7F)
                if matches!(style, CharStyle::Unicode) {
                    result.push('␡');
                } else {
                    result.push_str("^?");
                }
            }
            '\u{200b}' => {
                // Zero-width space
                if matches!(style, CharStyle::Unicode) {
                    result.push_str("[ZWSP]");
                } else {
                    result.push_str("[ZWSP]");
                }
            }
            '\u{200c}' => {
                // Zero-width non-joiner
                if matches!(style, CharStyle::Unicode) {
                    result.push_str("[ZWNJ]");
                } else {
                    result.push_str("[ZWNJ]");
                }
            }
            '\u{200d}' => {
                // Zero-width joiner
                if matches!(style, CharStyle::Unicode) {
                    result.push_str("[ZWJ]");
                } else {
                    result.push_str("[ZWJ]");
                }
            }
            '\u{feff}' => {
                // Zero-width no-break space (BOM)
                if matches!(style, CharStyle::Unicode) {
                    result.push_str("[BOM]");
                } else {
                    result.push_str("[BOM]");
                }
            }
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_unprintable_unicode() {
        let input = "hello\tworld\n";
        let result = show_unprintable(input, CharStyle::Unicode);
        assert_eq!(result, "hello→world␊\n");
    }

    #[test]
    fn test_show_unprintable_caret() {
        let input = "hello\tworld\n";
        let result = show_unprintable(input, CharStyle::Caret);
        assert_eq!(result, "hello^Iworld$\n");
    }

    #[test]
    fn test_spaces_to_middle_dot() {
        let input = "hello world";
        let result = show_unprintable(input, CharStyle::Unicode);
        assert_eq!(result, "hello·world");
    }

    #[test]
    fn test_carriage_return() {
        let input = "hello\rworld";
        let result = show_unprintable(input, CharStyle::Unicode);
        assert_eq!(result, "hello↵world");
    }

    #[test]
    fn test_escape_character() {
        let input = "start\x1bend";
        let result = show_unprintable(input, CharStyle::Unicode);
        assert_eq!(result, "start␛end");
    }

    #[test]
    fn test_null_character() {
        let input = "start\0end";
        let result = show_unprintable(input, CharStyle::Unicode);
        assert_eq!(result, "start␀end");
    }

    #[test]
    fn test_other_control_chars() {
        let input = "start\x01\x02\x03end";
        let result = show_unprintable(input, CharStyle::Unicode);
        assert_eq!(result, "start␁␂␃end");
    }

    #[test]
    fn test_del_character() {
        let input = "start\x7fend";
        let result = show_unprintable(input, CharStyle::Unicode);
        assert_eq!(result, "start␡end");
    }
}
