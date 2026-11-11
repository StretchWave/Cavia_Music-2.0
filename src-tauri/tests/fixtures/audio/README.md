# Embedded lyrics audio fixtures

These are minimal, silent, 0.6-second audio containers. They exist only so the
embedded-lyrics tests can exercise the real `lofty` read path
(`Probe::open(...).read()` → tag iteration) on a valid container of each format.
No user audio is ever read or modified, and no fixture contains real or
copyrighted lyrics.

## Generation

Produced once with ffmpeg (any recent build) and committed:

```sh
ffmpeg -f lavfi -i anullsrc=r=44100:cl=mono -t 0.6 -q:a 9 base.mp3
ffmpeg -f lavfi -i anullsrc=r=44100:cl=mono -t 0.6        base.flac
ffmpeg -f lavfi -i anullsrc=r=44100:cl=mono -t 0.6 -c:a aac base.m4a
```

Verify with:

```sh
ffprobe -v error -show_entries format=format_name,duration -of default=nw=1 base.mp3
```

## How tests use them

Each test copies a base container to a unique file in the system temporary
directory, writes the tag under test with `lofty`'s writer, runs the production
`extract_embedded_lyrics` function against the copy, and deletes the copy via a
`Drop` guard. The committed fixtures are never modified.

Tag construction per format:

| Format | Frame/field | Constructed with |
| --- | --- | --- |
| MP3 | ID3v2 `USLT` | `UnsynchronizedTextFrame::new(TextEncoding::UTF8, b"eng", "", text)` |
| MP3 | ID3v2 `SYLT` (ms) | `SynchronizedTextFrame::as_bytes()` wrapped in `BinaryFrame` with id `SYLT` |
| MP3 | ID3v2 `SYLT` (MPEG) | Same, with `TimestampFormat::MPEG` — documented as unsupported by the reader |
| FLAC | Vorbis Comment `LYRICS` | `Tag::insert_text(ItemKey::Lyrics, ...)` on `TagType::VorbisComments` |
| M4A | MP4 `©lyr` | `Tag::insert_text(ItemKey::Lyrics, ...)` on `TagType::Mp4Ilst` |

`SYLT` is written as a binary frame because `lofty` 0.25 exposes
`SynchronizedTextFrame` as a parser/encoder rather than an insertable tag frame;
the reader in `embedded.rs` sees it as `Frame::Binary` and parses it with
`SynchronizedTextFrame::parse`, which is exactly the documented production path.
