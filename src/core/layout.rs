use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn strip_ansi(input: &str) -> String {
    if !input.as_bytes().contains(&b'\x1b') {
        return input.to_string();
    }

    let mut output = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut idx = 0;

    while idx < chars.len() {
        if chars[idx] == '\u{1b}' {
            idx += 1;
            if idx >= chars.len() {
                break;
            }

            match chars[idx] {
                '[' => {
                    idx += 1;
                    while idx < chars.len() {
                        let ch = chars[idx];
                        idx += 1;
                        if ('@'..='~').contains(&ch) {
                            break;
                        }
                    }
                }
                ']' => {
                    idx += 1;
                    while idx < chars.len() {
                        let ch = chars[idx];
                        idx += 1;
                        if ch == '\u{7}' {
                            break;
                        }
                        if ch == '\u{1b}' && idx < chars.len() && chars[idx] == '\\' {
                            idx += 1;
                            break;
                        }
                    }
                }
                _ => {
                    idx += 1;
                }
            }

            continue;
        }

        output.push(chars[idx]);
        idx += 1;
    }

    output
}

pub fn wrap_ansi_for_zsh(input: &str) -> String {
    wrap_ansi_generic(input, "%{", "%}", true)
}

pub fn wrap_ansi_for_bash(input: &str) -> String {
    wrap_ansi_generic(input, "\\[", "\\]", false)
}

fn wrap_ansi_generic(
    input: &str,
    start_mark: &str,
    end_mark: &str,
    escape_percent: bool,
) -> String {
    let mut output = String::with_capacity(input.len() + 32);
    let chars: Vec<char> = input.chars().collect();
    let mut idx = 0;

    while idx < chars.len() {
        if chars[idx] == '\u{1b}' {
            let start = idx;
            idx += 1;
            if idx >= chars.len() {
                break;
            }

            match chars[idx] {
                '[' => {
                    idx += 1;
                    while idx < chars.len() {
                        let ch = chars[idx];
                        idx += 1;
                        if ('@'..='~').contains(&ch) {
                            break;
                        }
                    }

                    let sequence: String = chars[start..idx].iter().collect();
                    output.push_str(start_mark);
                    output.push_str(sequence.as_str());
                    output.push_str(end_mark);
                    continue;
                }
                ']' => {
                    idx += 1;
                    while idx < chars.len() {
                        let ch = chars[idx];
                        idx += 1;
                        if ch == '\u{7}' {
                            break;
                        }
                        if ch == '\u{1b}' && idx < chars.len() && chars[idx] == '\\' {
                            idx += 1;
                            break;
                        }
                    }

                    let sequence: String = chars[start..idx].iter().collect();
                    output.push_str(start_mark);
                    output.push_str(sequence.as_str());
                    output.push_str(end_mark);
                    continue;
                }
                _ => {
                    let sequence: String = chars[start..idx + 1].iter().collect();
                    output.push_str(start_mark);
                    output.push_str(sequence.as_str());
                    output.push_str(end_mark);
                    idx += 1;
                    continue;
                }
            }
        }

        if escape_percent && chars[idx] == '%' {
            output.push_str("%%");
        } else {
            output.push(chars[idx]);
        }
        idx += 1;
    }

    output
}

fn is_zero_width(ch: char) -> bool {
    matches!(
        ch,
        '\u{200b}'..='\u{200f}' // ZWSP, ZWNJ, ZWJ, LRM, RLM
            | '\u{2060}'        // word joiner
            | '\u{feff}'        // zero-width no-break space / BOM
            | '\u{fe00}'..='\u{fe0f}' // variation selectors
            | '\u{e0100}'..='\u{e01ef}' // variation selectors supplement
    )
}

/// Remove zero-width formatting code points that must never contribute to the
/// printable width of a prompt segment.
///
/// Emoji ZWJ sequences, variation selectors and bidi marks are invisible in
/// every terminal. If they were counted, right-aligned text (`RPROMPT`) would
/// drift by a variable number of columns and corrupt the layout. Combining
/// marks are already reported as width 0 by `unicode-width`, so this only drops
/// the "invisible but width-attributed" joiners.
pub fn strip_zero_width(input: &str) -> String {
    if !input.chars().any(is_zero_width) {
        return input.to_string();
    }
    input.chars().filter(|ch| !is_zero_width(*ch)).collect()
}

/// Printable width of `input`, ignoring ANSI escapes *and* zero-width marks.
pub fn visible_width(input: &str) -> usize {
    let has_escape = input.as_bytes().contains(&b'\x1b');
    let has_zero_width = input.chars().any(is_zero_width);

    if !has_escape && !has_zero_width {
        return UnicodeWidthStr::width(input);
    }

    let stripped = if has_escape {
        strip_ansi(input)
    } else {
        input.to_string()
    };
    UnicodeWidthStr::width(strip_zero_width(&stripped).as_str())
}

pub fn truncate_plain_to_width(input: &str, max_width: usize) -> String {
    if UnicodeWidthStr::width(input) <= max_width {
        return input.to_string();
    }

    if max_width == 0 {
        return String::new();
    }

    if max_width <= 3 {
        return take_prefix_by_width(input, max_width);
    }

    let mut out = String::new();
    let mut used = 0;

    for ch in input.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + w + 3 > max_width {
            break;
        }
        out.push(ch);
        used += w;
    }

    out.push_str("...");
    out
}

pub fn take_prefix_by_width(input: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut out = String::new();
    let mut used = 0;

    for ch in input.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + w > max_width {
            break;
        }
        out.push(ch);
        used += w;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_width_ignores_ansi() {
        let styled = "\x1b[1;32mcode\x1b[0m";
        assert_eq!(visible_width(styled), 4);
    }

    #[test]
    fn visible_width_ignores_zero_width_joiners() {
        // ZWJ, zero-width space and variation selectors must add no columns.
        assert_eq!(visible_width("\u{1f468}\u{200d}\u{1f469}"), 4);
        assert_eq!(visible_width("\u{200b}"), 0);
        assert_eq!(visible_width("\u{fe0f}"), 0);
        assert_eq!(visible_width("\u{2764}\u{fe0f}"), visible_width("\u{2764}"));
    }

    #[test]
    fn strip_zero_width_removes_variation_selectors() {
        let with_selector = "\u{2764}\u{fe0f}";
        assert_eq!(strip_zero_width(with_selector), "\u{2764}");
    }

    #[test]
    fn wrap_ansi_for_zsh_wraps_escape_sequences() {
        let wrapped = wrap_ansi_for_zsh("\x1b[32mhi\x1b[0m");
        assert_eq!(wrapped, "%{\x1b[32m%}hi%{\x1b[0m%}");
    }

    #[test]
    fn truncate_plain_adds_ellipsis_only_when_needed() {
        assert_eq!(truncate_plain_to_width("short", 10), "short");
        assert_eq!(truncate_plain_to_width("truncated", 6), "tru...");
    }
}
