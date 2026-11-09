use lofty::picture::PictureType;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use std::path::Path;

pub struct ExtractedMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub duration_ms: i64,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub release_year: Option<i32>,
    pub genre: Option<String>,
    pub picture_data: Option<(Vec<u8>, String)>, // (bytes, extension)
}

pub fn extract_metadata(path: &Path) -> Result<ExtractedMetadata, String> {
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown Title")
        .to_string();

    let probe_res = Probe::open(path)
        .map_err(|e| format!("Failed to probe file: {e}"))
        .and_then(|p| p.read().map_err(|e| format!("Failed to read tags: {e}")));

    match probe_res {
        Ok(tagged_file) => {
            let properties = tagged_file.properties();
            let duration_ms = properties.duration().as_millis() as i64;
            let tag = tagged_file
                .primary_tag()
                .or_else(|| tagged_file.first_tag());

            let mut title = file_stem;
            let mut artist = String::new();
            let mut album = String::new();
            let mut album_artist = None;
            let mut track_number = None;
            let mut disc_number = None;
            let mut release_year = None;
            let mut genre = None;
            let mut picture_data = None;

            if let Some(tag) = tag {
                if let Some(t) = tag.title() {
                    let trimmed = t.trim();
                    if !trimmed.is_empty() {
                        title = trimmed.to_string();
                    }
                }
                if let Some(a) = tag.artist() {
                    let trimmed = a.trim();
                    if !trimmed.is_empty() {
                        artist = trimmed.to_string();
                    }
                }
                if let Some(al) = tag.album() {
                    let trimmed = al.trim();
                    if !trimmed.is_empty() {
                        album = trimmed.to_string();
                    }
                }
                if let Some(aa) = tag.get_string(ItemKey::AlbumArtist) {
                    let trimmed = aa.trim();
                    if !trimmed.is_empty() {
                        album_artist = Some(trimmed.to_string());
                    }
                }
                track_number = tag.track().map(|n| n as i32);
                disc_number = tag.disk().map(|n| n as i32);
                if let Some(date_str) = tag
                    .get_string(ItemKey::RecordingDate)
                    .or_else(|| tag.get_string(ItemKey::OriginalReleaseDate))
                {
                    if let Some(year_part) = date_str.split('-').next() {
                        if let Ok(y) = year_part.trim().parse::<i32>() {
                            release_year = Some(y);
                        }
                    }
                }
                if let Some(g) = tag.genre() {
                    let trimmed = g.trim();
                    if !trimmed.is_empty() {
                        genre = Some(trimmed.to_string());
                    }
                }

                let pictures = tag.pictures();
                let cover = pictures
                    .iter()
                    .find(|p| p.pic_type() == PictureType::CoverFront)
                    .or_else(|| pictures.first());

                if let Some(pic) = cover {
                    let mime = pic.mime_type().map(|m| m.as_str()).unwrap_or("");
                    let ext = match mime {
                        "image/png" => "png",
                        "image/webp" => "webp",
                        "image/gif" => "gif",
                        _ => "jpg",
                    };
                    picture_data = Some((pic.data().to_vec(), ext.to_string()));
                }
            }

            Ok(ExtractedMetadata {
                title,
                artist,
                album,
                album_artist,
                duration_ms,
                track_number,
                disc_number,
                release_year,
                genre,
                picture_data,
            })
        }
        Err(_) => {
            // Graceful fallback for audio files with missing, non-standard, or unparseable metadata tags
            Ok(ExtractedMetadata {
                title: file_stem,
                artist: String::new(),
                album: String::new(),
                album_artist: None,
                duration_ms: 0,
                track_number: None,
                disc_number: None,
                release_year: None,
                genre: None,
                picture_data: None,
            })
        }
    }
}
