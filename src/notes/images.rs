//! Images pasted or dropped into a note (0.2.10): files beside the DB in `attachments/notes/<note id>/`, the way meeting
//! screenshots are (`meetings::shots`). The note body holds only `![image](/attachments/notes/<id>/<name>)`.
use rusqlite::Connection;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Bigger than any Retina full-screen PNG; a cap so a stray upload can't fill the disk.
pub const MAX_BYTES: usize = 25 * 1024 * 1024;
pub const REFUSED: &str = "Paste a PNG, JPEG, GIF or WebP image.";

/// `<db dir>/attachments/notes`; None for an in-memory DB (nothing to store next to).
pub fn root(conn: &Connection) -> Option<PathBuf> {
    let db = conn.path().filter(|p| !p.is_empty())?;
    Some(Path::new(db).parent()?.join("attachments").join("notes"))
}

/// The extension for what the bytes ARE (magic number), never what the client's header claims (D2). SVG is not here on purpose:
/// an uploaded .svg opened directly from our origin would run its script (D1).
fn ext(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if b.starts_with(b"\xFF\xD8\xFF") {
        Some("jpg")
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some("gif")
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// The `Content-Type` for a served name (`None` = not a name `save` writes).
pub fn content_type(name: &str) -> Option<&'static str> {
    Some(match name.rsplit_once('.')?.1 {
        "png" => "image/png",
        "jpg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    })
}

/// A file name this module wrote: `YYYYMMDD-HHMMSS.ext` or `YYYYMMDD-HHMMSS-N.ext`. Anything else is refused when served, which
/// also rules out `..` and separators.
pub fn valid_name(f: &str) -> bool {
    let Some((stem, _)) = f.rsplit_once('.') else {
        return false;
    };
    let mut parts = stem.splitn(3, '-');
    let digits = |s: Option<&str>, len: usize| {
        s.is_some_and(|s| s.len() == len && s.bytes().all(|b| b.is_ascii_digit()))
    };
    digits(parts.next(), 8)
        && digits(parts.next(), 6)
        && parts
            .next()
            .is_none_or(|n| !n.is_empty() && n.len() <= 3 && n.bytes().all(|b| b.is_ascii_digit()))
        && content_type(f).is_some()
}

/// Saves one image for note `id`, named by the local time `stamp` (`YYYYMMDD-HHMMSS`); returns the file name. Never overwrites:
/// a second image in the same second gets `-2`, `-3`, … (`create_new` makes the check and the create one step).
pub fn save(root: &Path, id: i64, bytes: &[u8], stamp: &str) -> Result<String, String> {
    if bytes.len() > MAX_BYTES {
        return Err("Image is too big (25 MB at most).".into());
    }
    let ext = ext(bytes).ok_or(REFUSED)?;
    let dir = root.join(id.to_string());
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut name = format!("{stamp}.{ext}");
    let mut n = 2;
    loop {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dir.join(&name))
        {
            Ok(mut f) => {
                f.write_all(bytes).map_err(|e| e.to_string())?;
                return Ok(name);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                name = format!("{stamp}-{n}.{ext}");
                n += 1;
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_decides_and_names_are_strict() {
        assert_eq!(ext(b"\x89PNG\r\n\x1a\nx"), Some("png"));
        assert_eq!(ext(b"\xFF\xD8\xFF\xE0x"), Some("jpg"));
        assert_eq!(ext(b"GIF89a.."), Some("gif"));
        assert_eq!(ext(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(ext(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"), None);
        assert_eq!(ext(b"RIFF\0\0\0\0WAVEfmt "), None);
        assert!(valid_name("20261117-101500.png") && valid_name("20261117-101500-2.webp"));
        for bad in [
            "../x.png",
            "x.png",
            "20261117-101500.svg",
            "20261117-101500-.png",
            "20261117-101500-1234.png",
            "20261117-101500.png.html",
            ".png",
        ] {
            assert!(!valid_name(bad), "{bad}");
        }
    }

    #[test]
    fn a_second_image_in_the_same_second_gets_its_own_name() {
        let dir = std::env::temp_dir().join(format!("atharu-img-unit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let png = b"\x89PNG\r\n\x1a\nA";
        let a = save(&dir, 3, png, "20261117-101500").unwrap();
        let b = save(&dir, 3, b"\x89PNG\r\n\x1a\nB", "20261117-101500").unwrap();
        let c = save(&dir, 3, png, "20261117-101500").unwrap();
        assert_eq!(
            (a.as_str(), b.as_str(), c.as_str()),
            (
                "20261117-101500.png",
                "20261117-101500-2.png",
                "20261117-101500-3.png"
            )
        );
        assert_eq!(
            std::fs::read(dir.join("3").join(&a)).unwrap(),
            png,
            "the first is not overwritten"
        );
        assert!([&a, &b, &c].into_iter().all(|n| valid_name(n)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
