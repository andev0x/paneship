//! Terminal layout primitives: ANSI-aware width measurement, truncation and
//! shell escape wrapping.
//!
//! Every measurement in the prompt funnels through [`visible_width`]. It is the
//! single source of truth for column accounting, which keeps the left and right
//! regions aligned even when a path shrinks after `cd`, when a segment contains
//! Nerd Font glyphs, or when an explicit user override inserts an ANSI sequence.
//!
//! The scanner is byte oriented. Escape sequences are pure ASCII, and plain
//! runs are measured with [`UnicodeWidthStr`] without materialising an
//! intermediate `String`, so even styled segments avoid the `Vec<char>`
//! allocation a naive parser would need.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const ESC: u8 = 0x1b;

/// Byte length of the escape sequence starting at `start`.
///
/// `start` must point at an `ESC` byte. Unterminated sequences deliberately
/// consume the rest of the input so a malformed prompt can never be counted as
/// visible columns.
fn escape_len(bytes: &[u8], start: usize) -> usize {
    debug_assert_eq!(bytes.get(start), Some(&ESC));

    let mut idx = start + 1;
    if idx >= bytes.len() {
        return idx - start;
    }

    match bytes[idx] {
        // CSI: ESC [ … final-byte(@..~)
        b'[' => {
            idx += 1;
            while idx < bytes.len() {
                let byte = bytes[idx];
                idx += 1;
                if (0x40..=0x7e).contains(&byte) {
                    break;
                }
            }
        }
        // OSC: ESC ] … BEL | ST
        b']' => {
            idx += 1;
            while idx < bytes.len() {
                match bytes[idx] {
                    0x07 => {
                        idx += 1;
                        break;
                    }
                    ESC if bytes.get(idx + 1) == Some(&b'\\') => {
                        idx += 2;
                        break;
                    }
                    _ => idx += 1,
                }
            }
        }
        // Two-byte escape. Skip the whole UTF-8 scalar after ESC when present.
        _ => {
            idx += 1;
            while idx < bytes.len() && (bytes[idx] & 0b1100_0000) == 0b1000_0000 {
                idx += 1;
            }
        }
    }

    idx - start
}

pub fn strip_ansi(input: &str) -> String {
    if !input.as_bytes().contains(&ESC) {
        return input.to_string();
    }

    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut copy_start = 0;
    let mut idx = 0;

    while idx < bytes.len() {
        if bytes[idx] == ESC {
            if copy_start < idx {
                output.push_str(&input[copy_start..idx]);
            }
            idx += escape_len(bytes, idx);
            copy_start = idx;
        } else {
            idx += 1;
        }
    }

    if copy_start < bytes.len() {
        output.push_str(&input[copy_start..]);
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
    let bytes = input.as_bytes();
    let mut plain_start = 0;
    let mut idx = 0;

    while idx < bytes.len() {
        if bytes[idx] == ESC {
            if plain_start < idx {
                push_plain(&mut output, &input[plain_start..idx], escape_percent);
            }

            let len = escape_len(bytes, idx);
            output.push_str(start_mark);
            output.push_str(&input[idx..idx + len]);
            output.push_str(end_mark);

            idx += len;
            plain_start = idx;
        } else {
            idx += 1;
        }
    }

    if plain_start < bytes.len() {
        push_plain(&mut output, &input[plain_start..], escape_percent);
    }

    output
}

fn push_plain(output: &mut String, segment: &str, escape_percent: bool) {
    if !escape_percent {
        output.push_str(segment);
        return;
    }

    for ch in segment.chars() {
        if ch == '%' {
            output.push_str("%%");
        } else {
            output.push(ch);
        }
    }
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
#[cfg_attr(not(test), allow(dead_code))]
pub fn strip_zero_width(input: &str) -> String {
    if !input.chars().any(is_zero_width) {
        return input.to_string();
    }
    input.chars().filter(|ch| !is_zero_width(*ch)).collect()
}

/// Printable width of `input`, ignoring ANSI escapes *and* zero-width marks.
///
/// This is allocation-free when the input contains neither escapes nor
/// zero-width scalars (the overwhelmingly common path).
pub fn visible_width(input: &str) -> usize {
    let bytes = input.as_bytes();
    let has_escape = bytes.contains(&ESC);

    if !has_escape && !input.chars().any(is_zero_width) {
        return UnicodeWidthStr::width(input);
    }

    // Measure contiguous plain-text runs with `UnicodeWidthStr` so contextual
    // widths (combining marks, emoji presentation) stay correct, while still
    // dropping zero-width joiners and splitting around escape sequences.
    let mut width = 0;
    let mut chunk_start = 0;
    let mut idx = 0;

    while idx < bytes.len() {
        if bytes[idx] == ESC {
            if chunk_start < idx {
                width += UnicodeWidthStr::width(&input[chunk_start..idx]);
            }
            idx += escape_len(bytes, idx);
            chunk_start = idx;
            continue;
        }

        let char_start = idx;
        let Some(ch) = input[idx..].chars().next() else {
            break;
        };
        idx += ch.len_utf8();

        if is_zero_width(ch) {
            if chunk_start < char_start {
                width += UnicodeWidthStr::width(&input[chunk_start..char_start]);
            }
            chunk_start = idx;
        }
    }

    if chunk_start < bytes.len() {
        width += UnicodeWidthStr::width(&input[chunk_start..]);
    }

    width
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
    fn visible_width_handles_osc_sequences_and_unicode() {
        let title = "\x1b]0;paneship\x07❯ ~/dev";
        assert_eq!(visible_width(title), visible_width("❯ ~/dev"));
    }

    #[test]
    fn visible_width_handles_unterminated_escapes() {
        // The visible prefix still counts; the dangling sequence does not.
        assert_eq!(visible_width("path\x1b[31"), 4);
        assert_eq!(visible_width("\x1b"), 0);
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
    fn wrap_ansi_for_bash_wraps_escape_sequences() {
        let wrapped = wrap_ansi_for_bash("\x1b[32mhi\x1b[0m");
        assert_eq!(wrapped, "\\[\x1b[32m\\]hi\\[\x1b[0m\\]");
    }

    #[test]
    fn wrap_ansi_for_zsh_escapes_literal_percent() {
        assert_eq!(wrap_ansi_for_zsh("100%"), "100%%");
    }

    #[test]
    fn truncate_plain_adds_ellipsis_only_when_needed() {
        assert_eq!(truncate_plain_to_width("short", 10), "short");
        assert_eq!(truncate_plain_to_width("truncated", 6), "tru...");
    }
}
