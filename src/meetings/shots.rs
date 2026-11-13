//! Meeting screenshots (T89, D57): PNG files beside the DB in `attachments/meetings/<id>/`, listed on the
//! meeting page and in `atharu meeting`. Files, not rows: no schema change, and the notes text is left alone
//! (its box autosaves the whole text, so a line added behind it would be overwritten).
use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// Bigger than any Retina full-screen PNG; a cap so a stray upload can't fill the disk.
pub const MAX_BYTES: usize = 25 * 1024 * 1024;
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

/// `<db dir>/attachments/meetings`; None for an in-memory DB (nothing to store next to).
pub fn root(conn: &Connection) -> Option<PathBuf> {
    let db = conn.path().filter(|p| !p.is_empty())?;
    Some(Path::new(db).parent()?.join("attachments").join("meetings"))
}

/// A file name this module wrote: `HHMMSS.png` or `HHMMSS-N.png`. Anything else is refused when served,
/// which also rules out `..` and separators.
pub fn valid_name(f: &str) -> bool {
    let Some(stem) = f.strip_suffix(".png") else {
        return false;
    };
    let (time, n) = stem.split_once('-').unwrap_or((stem, "1"));
    time.len() == 6
        && time.bytes().all(|b| b.is_ascii_digit())
        && !n.is_empty()
        && n.len() <= 3
        && n.bytes().all(|b| b.is_ascii_digit())
}

/// Saves one PNG for meeting `id`, named by the local time `hhmmss`; returns the file name.
pub fn save(root: &Path, id: i64, png: &[u8], hhmmss: &str) -> Result<String, String> {
    if png.len() > MAX_BYTES {
        return Err("Screenshot is too big.".into());
    }
    if !png.starts_with(PNG_MAGIC) {
        return Err("Not a PNG.".into());
    }
    let dir = root.join(id.to_string());
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut name = format!("{hhmmss}.png");
    let mut n = 2;
    while dir.join(&name).exists() {
        name = format!("{hhmmss}-{n}.png");
        n += 1;
    }
    std::fs::write(dir.join(&name), png).map_err(|e| e.to_string())?;
    Ok(name)
}

/// Meeting `id`'s screenshots, oldest first (the names sort by time).
pub fn list(root: &Path, id: i64) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(root.join(id.to_string())) else {
        return Vec::new();
    };
    let mut v: Vec<String> = rd
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|f| valid_name(f))
        .collect();
    // By time, then the -N repeat ('-' sorts before '.', so a plain sort puts `-2` first).
    v.sort_by_cached_key(|f| {
        let stem = f.trim_end_matches(".png");
        let (t, n) = stem.split_once('-').unwrap_or((stem, "1"));
        (t.to_string(), n.parse::<u32>().unwrap_or(1))
    });
    v
}

/// `HH:MM` of a file name, for its caption.
pub fn label(f: &str) -> String {
    format!("{}:{}", &f[0..2], &f[2..4])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_saves_and_refuses() {
        assert!(valid_name("112130.png") && valid_name("112130-2.png"));
        for bad in [
            "../x.png",
            "112130.jpg",
            "11213.png",
            "112130-.png",
            "a/112130.png",
            "112130-2-3.png",
        ] {
            assert!(!valid_name(bad), "{bad}");
        }
        let root = std::env::temp_dir().join(format!("atharu-shots-{}", std::process::id()));
        let png = [PNG_MAGIC, b"rest"].concat();
        assert_eq!(save(&root, 7, &png, "112130").unwrap(), "112130.png");
        assert_eq!(save(&root, 7, &png, "112130").unwrap(), "112130-2.png");
        assert!(save(&root, 7, b"GIF89a", "112131").is_err());
        assert_eq!(list(&root, 7), ["112130.png", "112130-2.png"]);
        assert_eq!(label("112130.png"), "11:21");
        let _ = std::fs::remove_dir_all(&root);
    }
}
