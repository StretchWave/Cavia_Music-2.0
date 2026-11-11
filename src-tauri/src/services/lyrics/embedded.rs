use lofty::id3::v2::{Frame, Id3v2Tag, SynchronizedTextFrame, TimestampFormat};
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::{ItemKey, TagType};
use std::path::Path;

use super::parser::is_likely_lrc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbeddedLyricsResult {
    Synced { raw_lrc: String },
    Plain { text: String },
    Both { raw_lrc: String, plain: String },
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LyricsContentKind {
    Empty,
    Plain,
    Lrc,
}

/// Shared classification for every embedded lyrics source.
///
/// The same rule is applied to ID3v2 USLT, generic `ItemKey::Lyrics`
/// (Vorbis Comments `LYRICS`, MP4 `©lyr`) and `ItemKey::UnsyncLyrics`
/// (`UNSYNCEDLYRICS`): whitespace-only content is ignored, content containing
/// at least one valid LRC timestamp is synchronized, everything else is plain.
/// A malformed timestamp (for example `[00:99.00]` or `[xx:yy]`) does not make
/// the payload synchronized.
fn classify_lyrics_content(content: &str) -> LyricsContentKind {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        LyricsContentKind::Empty
    } else if is_likely_lrc(trimmed) {
        LyricsContentKind::Lrc
    } else {
        LyricsContentKind::Plain
    }
}

/// Applies classified content to the accumulators. The original text
/// (including every LRC timestamp) is preserved; only surrounding whitespace is
/// trimmed. The first source of each kind wins, matching the previous
/// precedence rules.
fn apply_lyrics_content(
    content: &str,
    found_synced_lrc: &mut Option<String>,
    found_plain: &mut Option<String>,
) {
    let trimmed = content.trim();
    match classify_lyrics_content(trimmed) {
        LyricsContentKind::Empty => {}
        LyricsContentKind::Lrc => {
            if found_synced_lrc.is_none() {
                *found_synced_lrc = Some(trimmed.to_string());
            }
        }
        LyricsContentKind::Plain => {
            if found_plain.is_none() {
                *found_plain = Some(trimmed.to_string());
            }
        }
    }
}

/// Extracts embedded lyrics from an audio file.
/// Inspects ID3v2 (USLT, SYLT), Vorbis Comments (LYRICS, UNSYNCEDLYRICS), and MP4 (©lyr).
/// Returns plain, synchronized, both, or none. Never panics or modifies files.
pub fn extract_embedded_lyrics(path: &Path) -> EmbeddedLyricsResult {
    if !path.exists() || !path.is_file() {
        return EmbeddedLyricsResult::None;
    }

    let tagged_file_res = Probe::open(path).and_then(|p| p.read());

    let tagged_file = match tagged_file_res {
        Ok(tf) => tf,
        Err(_) => return EmbeddedLyricsResult::None,
    };

    let mut found_synced_lrc: Option<String> = None;
    let mut found_plain: Option<String> = None;

    // 1. First inspect format-specific ID3v2 tags for SYLT and USLT frames
    if let Some(id3_tag) = tagged_file.tag(TagType::Id3v2) {
        let id3v2: Id3v2Tag = id3_tag.clone().into();
        for frame in id3v2.iter() {
            if frame.id_str() == "SYLT" {
                if let Frame::Binary(bin) = frame {
                    if let Ok(sylt) = SynchronizedTextFrame::parse(&bin.data, bin.flags()) {
                        // MPEG-frame timestamps are not converted: without the
                        // exact MPEG frame duration they cannot be mapped to
                        // milliseconds reliably. Such frames are ignored rather
                        // than guessed.
                        if sylt.timestamp_format == TimestampFormat::MS {
                            let mut lrc_lines = Vec::new();
                            let mut plain_lines = Vec::new();

                            for (ms_u32, text) in &sylt.content {
                                let trimmed = text.trim();
                                if trimmed.is_empty() {
                                    continue;
                                }
                                let ms = *ms_u32 as i64;
                                let minutes = ms / 60000;
                                let seconds = (ms % 60000) / 1000;
                                // SYLT stores milliseconds; LRC here uses
                                // hundredths, so sub-10 ms precision is
                                // intentionally truncated.
                                let hundredths = (ms % 1000) / 10;
                                lrc_lines.push(format!(
                                    "[{:02}:{:02}.{:02}]{}",
                                    minutes, seconds, hundredths, trimmed
                                ));
                                plain_lines.push(trimmed.to_string());
                            }

                            if !lrc_lines.is_empty() {
                                found_synced_lrc = Some(lrc_lines.join("\n"));
                                if found_plain.is_none() && !plain_lines.is_empty() {
                                    found_plain = Some(plain_lines.join("\n"));
                                }
                            }
                        }
                    }
                }
            } else if frame.id_str() == "USLT" {
                if let Frame::UnsynchronizedText(unsync) = frame {
                    // D4: USLT is not assumed to be plain text. A frame that
                    // contains valid LRC timestamps must be treated as synced,
                    // exactly like generic lyrics metadata.
                    apply_lyrics_content(&unsync.content, &mut found_synced_lrc, &mut found_plain);
                }
            }
        }
    }

    // 2. Inspect generic tag items (covers Vorbis Comments, FLAC, MP4 ©lyr, APE, and generic ID3 tags)
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());
    if let Some(tag) = tag {
        if let Some(lyrics_val) = tag.get_string(ItemKey::Lyrics) {
            apply_lyrics_content(lyrics_val, &mut found_synced_lrc, &mut found_plain);
        }

        if let Some(unsync_val) = tag.get_string(ItemKey::UnsyncLyrics) {
            apply_lyrics_content(unsync_val, &mut found_synced_lrc, &mut found_plain);
        }
    }

    match (found_synced_lrc, found_plain) {
        (Some(raw_lrc), Some(plain)) => EmbeddedLyricsResult::Both { raw_lrc, plain },
        (Some(raw_lrc), None) => EmbeddedLyricsResult::Synced { raw_lrc },
        (None, Some(text)) => EmbeddedLyricsResult::Plain { text },
        (None, None) => EmbeddedLyricsResult::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::TextEncoding;
    use lofty::config::WriteOptions;
    use lofty::id3::v2::{BinaryFrame, FrameId, SyncTextContentType, UnsynchronizedTextFrame};
    use lofty::tag::Tag;
    use std::borrow::Cow;
    use std::fs;
    use std::path::PathBuf;

    const PLAIN_LYRICS: &str = "First plain line\nSecond plain line";
    const LRC_LYRICS: &str = "[00:01.00]First synced line\n[00:02.50]Second synced line";

    struct TempAudio {
        path: PathBuf,
    }

    impl Drop for TempAudio {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    fn base_fixture(extension: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("audio")
            .join(format!("base.{extension}"))
    }

    /// Copies a committed, silent audio fixture to a unique temporary file.
    /// The source fixture is never modified; the guard removes the copy.
    fn temp_audio(extension: &str) -> TempAudio {
        let source = base_fixture(extension);
        assert!(
            source.exists(),
            "missing base fixture {}. Generate it with ffmpeg.",
            source.display()
        );
        let path = std::env::temp_dir().join(format!(
            "cavia_embedded_{}.{}",
            uuid::Uuid::new_v4(),
            extension
        ));
        fs::copy(&source, &path).expect("copy base fixture");
        TempAudio { path }
    }

    fn write_id3v2(path: &Path, frames: impl IntoIterator<Item = Frame<'static>>) -> Id3v2Tag {
        let mut tag = Id3v2Tag::new();
        for frame in frames {
            tag.insert(frame);
        }
        tag.save_to_path(path, WriteOptions::default())
            .expect("write ID3v2 tag");
        tag
    }

    fn uslt_frame(content: &str) -> Frame<'static> {
        UnsynchronizedTextFrame::new(TextEncoding::UTF8, *b"eng", "", content.to_string()).into()
    }

    fn sylt_frame(format: TimestampFormat, content: Vec<(u32, String)>) -> Frame<'static> {
        let frame = SynchronizedTextFrame::new(
            TextEncoding::UTF8,
            *b"eng",
            format,
            SyncTextContentType::Lyrics,
            None,
            content,
        );
        let bytes = frame
            .as_bytes(WriteOptions::default())
            .expect("encode SYLT frame");
        BinaryFrame::new(FrameId::Valid(Cow::Borrowed("SYLT")), bytes).into()
    }

    #[test]
    fn test_extract_missing_file_returns_none() {
        let path = PathBuf::from("non_existent_file_cavia_lyrics.flac");
        let res = extract_embedded_lyrics(&path);
        assert_eq!(res, EmbeddedLyricsResult::None);
    }

    #[test]
    fn test_uslt_plain_text_is_plain() {
        let fixture = temp_audio("mp3");
        write_id3v2(&fixture.path, [uslt_frame(PLAIN_LYRICS)]);

        let result = extract_embedded_lyrics(&fixture.path);
        assert_eq!(
            result,
            EmbeddedLyricsResult::Plain {
                text: PLAIN_LYRICS.to_string()
            }
        );
    }

    /// D4 regression: before the fix, a USLT frame containing valid LRC was
    /// returned as plain text because only generic lyrics keys were classified.
    #[test]
    fn test_uslt_with_lrc_is_synchronized() {
        let fixture = temp_audio("mp3");
        write_id3v2(&fixture.path, [uslt_frame(LRC_LYRICS)]);

        let result = extract_embedded_lyrics(&fixture.path);
        assert_eq!(
            result,
            EmbeddedLyricsResult::Synced {
                raw_lrc: LRC_LYRICS.to_string()
            }
        );
    }

    #[test]
    fn test_uslt_empty_content_is_ignored() {
        let fixture = temp_audio("mp3");
        write_id3v2(&fixture.path, [uslt_frame("   \n\t  ")]);

        let result = extract_embedded_lyrics(&fixture.path);
        assert_eq!(result, EmbeddedLyricsResult::None);
    }

    #[test]
    fn test_uslt_malformed_timestamps_are_plain() {
        let fixture = temp_audio("mp3");
        let malformed = "[00:99.00]Broken seconds\n[xx:yy]Not a timestamp\nStill a lyric";
        write_id3v2(&fixture.path, [uslt_frame(malformed)]);

        let result = extract_embedded_lyrics(&fixture.path);
        assert_eq!(
            result,
            EmbeddedLyricsResult::Plain {
                text: malformed.to_string()
            }
        );
    }

    #[test]
    fn test_sylt_millisecond_timestamps_are_converted_to_lrc() {
        let fixture = temp_audio("mp3");
        write_id3v2(
            &fixture.path,
            [sylt_frame(
                TimestampFormat::MS,
                vec![
                    (1000, "First synced line".into()),
                    (2549, "Second synced line".into()),
                ],
            )],
        );

        let result = extract_embedded_lyrics(&fixture.path);
        assert_eq!(
            result,
            EmbeddedLyricsResult::Both {
                raw_lrc: "[00:01.00]First synced line\n[00:02.54]Second synced line".to_string(),
                plain: "First synced line\nSecond synced line".to_string(),
            },
            "milliseconds must map to LRC hundredths (truncated)"
        );
    }

    #[test]
    fn test_sylt_mpeg_timestamps_are_not_converted() {
        let fixture = temp_audio("mp3");
        write_id3v2(
            &fixture.path,
            [sylt_frame(
                TimestampFormat::MPEG,
                vec![(100, "MPEG frame lyric".into())],
            )],
        );

        let result = extract_embedded_lyrics(&fixture.path);
        assert_eq!(
            result,
            EmbeddedLyricsResult::None,
            "MPEG-frame SYLT is documented as unsupported"
        );
    }

    #[test]
    fn test_uslt_and_sylt_can_coexist() {
        let fixture = temp_audio("mp3");
        write_id3v2(
            &fixture.path,
            [
                sylt_frame(TimestampFormat::MS, vec![(1000, "Synced from SYLT".into())]),
                uslt_frame(PLAIN_LYRICS),
            ],
        );

        let result = extract_embedded_lyrics(&fixture.path);
        assert_eq!(
            result,
            EmbeddedLyricsResult::Both {
                raw_lrc: "[00:01.00]Synced from SYLT".to_string(),
                // Existing precedence: SYLT's derived plain text is filled
                // first, so a later USLT does not replace it.
                plain: "Synced from SYLT".to_string(),
            }
        );
    }

    fn write_generic_tag(path: &Path, tag_type: TagType, text: &str) {
        let mut tag = Tag::new(tag_type);
        assert!(
            tag.insert_text(ItemKey::Lyrics, text.to_string()),
            "ItemKey::Lyrics must map to {tag_type:?}"
        );
        tag.save_to_path(path, WriteOptions::default())
            .expect("write generic lyrics tag");
    }

    #[test]
    fn test_flac_vorbis_lyrics_with_lrc_is_synchronized() {
        let fixture = temp_audio("flac");
        write_generic_tag(&fixture.path, TagType::VorbisComments, LRC_LYRICS);

        let result = extract_embedded_lyrics(&fixture.path);
        assert_eq!(
            result,
            EmbeddedLyricsResult::Synced {
                raw_lrc: LRC_LYRICS.to_string()
            }
        );
    }

    #[test]
    fn test_flac_vorbis_plain_lyrics_is_plain() {
        let fixture = temp_audio("flac");
        write_generic_tag(&fixture.path, TagType::VorbisComments, PLAIN_LYRICS);

        let result = extract_embedded_lyrics(&fixture.path);
        assert_eq!(
            result,
            EmbeddedLyricsResult::Plain {
                text: PLAIN_LYRICS.to_string()
            }
        );
    }

    #[test]
    fn test_mp4_lyrics_with_lrc_is_synchronized() {
        let fixture = temp_audio("m4a");
        write_generic_tag(&fixture.path, TagType::Mp4Ilst, LRC_LYRICS);

        let result = extract_embedded_lyrics(&fixture.path);
        assert_eq!(
            result,
            EmbeddedLyricsResult::Synced {
                raw_lrc: LRC_LYRICS.to_string()
            }
        );
    }

    #[test]
    fn test_mp4_plain_lyrics_is_plain() {
        let fixture = temp_audio("m4a");
        write_generic_tag(&fixture.path, TagType::Mp4Ilst, PLAIN_LYRICS);

        let result = extract_embedded_lyrics(&fixture.path);
        assert_eq!(
            result,
            EmbeddedLyricsResult::Plain {
                text: PLAIN_LYRICS.to_string()
            }
        );
    }
}
