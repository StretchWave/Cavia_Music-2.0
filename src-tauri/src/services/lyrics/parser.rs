use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsLine {
    pub timestamp_ms: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ParsedLrc {
    pub lines: Vec<LyricsLine>,
    pub offset_ms: i64,
    pub plain_text: String,
}

/// Detects if a text string contains LRC synchronized timestamp markers like `[00:12.34]`.
pub fn is_likely_lrc(content: &str) -> bool {
    let mut count = 0;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('[') {
            if let Some(bracket_end) = rest.find(']') {
                let tag = &rest[..bracket_end];
                if parse_timestamp_tag(tag).is_some() {
                    count += 1;
                    if count >= 2 {
                        return true;
                    }
                }
            }
        }
    }
    count >= 1
}

/// Parses an LRC timestamp string (without brackets) like "01:23.45", "01:23.456", "01:23.4", or "01:23".
/// Returns milliseconds.
fn parse_timestamp_tag(tag: &str) -> Option<i64> {
    let trimmed = tag.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Split on ':'
    let parts: Vec<&str> = trimmed.split(':').collect();
    if parts.len() == 2 {
        let minutes: i64 = parts[0].trim().parse().ok()?;
        if minutes < 0 || minutes > 599 {
            return None;
        }

        // Second part can be "ss", "ss.xx", "ss.xxx", etc.
        let sec_parts: Vec<&str> = parts[1].split('.').collect();
        if sec_parts.len() == 1 {
            let seconds: i64 = sec_parts[0].trim().parse().ok()?;
            if seconds < 0 || seconds > 59 {
                return None;
            }
            return Some((minutes * 60 + seconds) * 1000);
        } else if sec_parts.len() == 2 {
            let seconds: i64 = sec_parts[0].trim().parse().ok()?;
            if seconds < 0 || seconds > 59 {
                return None;
            }

            let frac_str = sec_parts[1].trim();
            if frac_str.is_empty()
                || frac_str.len() > 6
                || !frac_str.chars().all(|c| c.is_ascii_digit())
            {
                return None;
            }

            let ms: i64 = match frac_str.len() {
                1 => frac_str.parse::<i64>().ok()? * 100, // tenths
                2 => frac_str.parse::<i64>().ok()? * 10,  // hundredths
                3 => frac_str.parse::<i64>().ok()?,       // milliseconds
                _ => {
                    // Truncate/round to 3 decimal digits
                    frac_str[..3].parse::<i64>().ok()?
                }
            };

            return Some((minutes * 60 + seconds) * 1000 + ms);
        }
    } else if parts.len() == 3 {
        // [hh:mm:ss.xx]
        let hours: i64 = parts[0].trim().parse().ok()?;
        let minutes: i64 = parts[1].trim().parse().ok()?;
        if hours < 0 || hours > 99 || minutes < 0 || minutes > 59 {
            return None;
        }

        let sec_parts: Vec<&str> = parts[2].split('.').collect();
        let seconds: i64 = sec_parts[0].trim().parse().ok()?;
        if seconds < 0 || seconds > 59 {
            return None;
        }

        let ms = if sec_parts.len() == 2 {
            let frac_str = sec_parts[1].trim();
            if frac_str.is_empty() || !frac_str.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            match frac_str.len() {
                1 => frac_str.parse::<i64>().ok()? * 100,
                2 => frac_str.parse::<i64>().ok()? * 10,
                3 => frac_str.parse::<i64>().ok()?,
                _ => frac_str[..3].parse::<i64>().ok()?,
            }
        } else {
            0
        };

        return Some((hours * 3600 + minutes * 60 + seconds) * 1000 + ms);
    }

    None
}

/// Parses an offset metadata tag like `[offset:+500]` or `[offset:-200]` in milliseconds.
fn parse_offset_tag(line: &str) -> Option<i64> {
    let lower = line.to_lowercase();
    if let Some(idx) = lower.find("[offset:") {
        let rest = &lower[idx + 8..];
        if let Some(end) = rest.find(']') {
            let val = rest[..end].trim();
            return val.parse::<i64>().ok();
        }
    }
    None
}

/// Strips all `[tag:value]` and timestamp brackets `[00:12.34]` from LRC lines to produce clean plain lyrics.
pub fn lrc_to_plain_text(content: &str) -> String {
    let mut plain_lines = Vec::new();

    for line in content.lines() {
        let mut cur = line;
        // Strip leading metadata or timestamps
        while let Some(open) = cur.find('[') {
            if let Some(close) = cur[open..].find(']') {
                let after = &cur[open + close + 1..];
                cur = after;
            } else {
                break;
            }
        }
        let trimmed = cur.trim();
        if !trimmed.is_empty() {
            plain_lines.push(trimmed);
        }
    }

    plain_lines.join("\n")
}

/// Parses an LRC document into structured, timestamp-ordered lines and plain lyrics.
pub fn parse_lrc(content: &str) -> ParsedLrc {
    let mut raw_lines: Vec<(i64, usize, String)> = Vec::new();
    let mut offset_ms: i64 = 0;
    let mut plain_lines: Vec<String> = Vec::new();
    let mut line_seq = 0;

    for raw_line in content.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check for offset tag
        if let Some(off) = parse_offset_tag(trimmed) {
            offset_ms = off;
            continue;
        }

        // Check for other metadata tags like [ti:], [ar:], [al:], [by:], [re:], [ve:], [length:]
        if trimmed.starts_with('[') {
            if let Some(end) = trimmed.find(']') {
                let tag = &trimmed[1..end];
                if tag.contains(':') && parse_timestamp_tag(tag).is_none() {
                    // Metadata tag, skip from lyric lines
                    continue;
                }
            }
        }

        // Parse timestamps on this line. A single line can contain multiple timestamps:
        // e.g. [00:12.34][00:15.67]Repeat chorus
        let mut timestamps = Vec::new();
        let mut rest = trimmed;

        while rest.starts_with('[') {
            if let Some(end) = rest.find(']') {
                let tag = &rest[1..end];
                if let Some(ts) = parse_timestamp_tag(tag) {
                    timestamps.push(ts);
                    rest = rest[end + 1..].trim_start();
                } else {
                    // Not a valid timestamp tag
                    break;
                }
            } else {
                break;
            }
        }

        let lyric_text = rest.trim().to_string();

        if !timestamps.is_empty() {
            if !lyric_text.is_empty() {
                plain_lines.push(lyric_text.clone());
            }
            for ts in timestamps {
                raw_lines.push((ts, line_seq, lyric_text.clone()));
                line_seq += 1;
            }
        } else if !lyric_text.is_empty() {
            // Text line without timestamp
            plain_lines.push(lyric_text);
        }
    }

    // Apply offset and sort stably
    let mut lines = Vec::new();
    // Sort by timestamp ascending; for equal timestamps, sort by line_seq ascending (stable)
    raw_lines.sort_by(|a, b| match a.0.cmp(&b.0) {
        std::cmp::Ordering::Equal => a.1.cmp(&b.1),
        other => other,
    });

    for (ts, _, text) in raw_lines {
        // Apply offset, clamping to 0 if negative
        let adjusted_ts = (ts + offset_ms).max(0);
        lines.push(LyricsLine {
            timestamp_ms: adjusted_ts,
            text,
        });
    }

    let plain_text = if !plain_lines.is_empty() {
        plain_lines.join("\n")
    } else {
        lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    };

    ParsedLrc {
        lines,
        offset_ms,
        plain_text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_standard_two_decimal_lrc() {
        let lrc = r#"
[00:04.17]First line of lyrics
[00:08.50]Second line of lyrics
[00:12.92]Third line of lyrics
"#;
        let parsed = parse_lrc(lrc);
        assert_eq!(parsed.lines.len(), 3);
        assert_eq!(parsed.lines[0].timestamp_ms, 4170);
        assert_eq!(parsed.lines[0].text, "First line of lyrics");
        assert_eq!(parsed.lines[1].timestamp_ms, 8500);
        assert_eq!(parsed.lines[2].timestamp_ms, 12920);
        assert_eq!(parsed.offset_ms, 0);
    }

    #[test]
    fn test_parse_three_and_one_decimal_digits() {
        let lrc = r#"
[00:01.5]One decimal digit (500ms)
[00:02.123]Three decimal digits (123ms)
[00:03]Zero decimal digits
"#;
        let parsed = parse_lrc(lrc);
        assert_eq!(parsed.lines.len(), 3);
        assert_eq!(parsed.lines[0].timestamp_ms, 1500);
        assert_eq!(parsed.lines[1].timestamp_ms, 2123);
        assert_eq!(parsed.lines[2].timestamp_ms, 3000);
    }

    #[test]
    fn test_multiple_timestamps_per_line() {
        let lrc = "[00:10.00][00:20.00][00:30.00]Repeated chorus";
        let parsed = parse_lrc(lrc);
        assert_eq!(parsed.lines.len(), 3);
        assert_eq!(parsed.lines[0].timestamp_ms, 10000);
        assert_eq!(parsed.lines[0].text, "Repeated chorus");
        assert_eq!(parsed.lines[1].timestamp_ms, 20000);
        assert_eq!(parsed.lines[1].text, "Repeated chorus");
        assert_eq!(parsed.lines[2].timestamp_ms, 30000);
        assert_eq!(parsed.lines[2].text, "Repeated chorus");
    }

    #[test]
    fn test_offset_handling_positive_and_negative() {
        let lrc_pos = r#"
[offset:+500]
[00:01.00]Line with +500ms offset
"#;
        let parsed_pos = parse_lrc(lrc_pos);
        assert_eq!(parsed_pos.offset_ms, 500);
        assert_eq!(parsed_pos.lines[0].timestamp_ms, 1500);

        let lrc_neg = r#"
[offset:-300]
[00:01.00]Line with -300ms offset
[00:00.10]Line clamped to zero
"#;
        let parsed_neg = parse_lrc(lrc_neg);
        assert_eq!(parsed_neg.offset_ms, -300);
        // Sorted ascending: 100ms - 300ms -> clamped to 0 is first; 1000ms - 300ms -> 700ms is second
        assert_eq!(parsed_neg.lines[0].timestamp_ms, 0);
        assert_eq!(parsed_neg.lines[0].text, "Line clamped to zero");
        assert_eq!(parsed_neg.lines[1].timestamp_ms, 700);
        assert_eq!(parsed_neg.lines[1].text, "Line with -300ms offset");
    }

    #[test]
    fn test_metadata_tags_ignored_gracefully() {
        let lrc = r#"
[ti:Midnight Call]
[ar:Cavia Artist]
[al:Obsidian Vinyl]
[by:Cavia Transcriptionist]
[length:03:45]
[00:05.00]Actual lyric line
"#;
        let parsed = parse_lrc(lrc);
        assert_eq!(parsed.lines.len(), 1);
        assert_eq!(parsed.lines[0].text, "Actual lyric line");
        assert_eq!(parsed.lines[0].timestamp_ms, 5000);
    }

    #[test]
    fn test_unicode_and_calligraphy_lyrics() {
        let lrc = r#"
[00:01.00]夜の静寂に響く音 藏
[00:03.50]Élégie pour un rêve doré
[00:06.00]Café con música suave 🎶
"#;
        let parsed = parse_lrc(lrc);
        assert_eq!(parsed.lines.len(), 3);
        assert_eq!(parsed.lines[0].text, "夜の静寂に響く音 藏");
        assert_eq!(parsed.lines[1].text, "Élégie pour un rêve doré");
        assert_eq!(parsed.lines[2].text, "Café con música suave 🎶");
    }

    #[test]
    fn test_out_of_order_and_duplicate_timestamps_stable_sort() {
        let lrc = r#"
[00:15.00]Later line
[00:05.00]Early line A
[00:05.00]Early line B
[00:01.00]First line
"#;
        let parsed = parse_lrc(lrc);
        assert_eq!(parsed.lines.len(), 4);
        assert_eq!(parsed.lines[0].timestamp_ms, 1000);
        assert_eq!(parsed.lines[0].text, "First line");
        assert_eq!(parsed.lines[1].timestamp_ms, 5000);
        assert_eq!(parsed.lines[1].text, "Early line A");
        assert_eq!(parsed.lines[2].timestamp_ms, 5000);
        assert_eq!(parsed.lines[2].text, "Early line B");
        assert_eq!(parsed.lines[3].timestamp_ms, 15000);
        assert_eq!(parsed.lines[3].text, "Later line");
    }

    #[test]
    fn test_malformed_lines_handled_without_crash() {
        let lrc = r#"
[not:a:timestamp]
[9999:9999:9999]
[00:invalid]
This is just text without brackets
[00:02.00]Valid line
"#;
        let parsed = parse_lrc(lrc);
        assert_eq!(parsed.lines.len(), 1);
        assert_eq!(parsed.lines[0].timestamp_ms, 2000);
        assert_eq!(parsed.lines[0].text, "Valid line");
    }

    #[test]
    fn test_is_likely_lrc_detection() {
        assert!(is_likely_lrc("[00:01.23]Hello\n[00:04.56]World"));
        assert!(!is_likely_lrc(
            "Just some plain lyrics\nwithout any timestamps"
        ));
        assert!(!is_likely_lrc(
            "[ti:Song Title]\n[ar:Artist]\nPlain line without timestamp"
        ));
    }

    #[test]
    fn test_lrc_to_plain_text_stripping() {
        let lrc = r#"
[ti:Title]
[00:01.00]First line
[00:02.00][00:03.00]Second line
"#;
        let plain = lrc_to_plain_text(lrc);
        assert_eq!(plain, "First line\nSecond line");
    }
}
