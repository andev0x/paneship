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

pub fn visible_width(input: &str) -> usize {
    if !input.as_bytes().contains(&b'\x1b') {
        return UnicodeWidthStr::width(input);
    }

    UnicodeWidthStr::width(strip_ansi(input).as_str())
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
