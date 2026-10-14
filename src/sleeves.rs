//! Adding sleeve images to the simulator's `Sleeves` folder, from a file on disk or a link.
//!
//! The image is checked (it must be a JPEG or PNG the simulator can show) and copied under
//! a free name, never overwriting a sleeve that is already there.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use image::ImageFormat;

use crate::decks::{Sleeve, file_stem_for};

/// Largest image accepted, so a wrong link does not fill the disk.
const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;
/// Name of a sleeve whose source tells nothing useful.
const FALLBACK_NAME: &str = "Sleeve";

/// Where a sleeve image comes from, as the user typed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SleeveSource {
    Url(String),
    File(PathBuf),
}

impl SleeveSource {
    /// A link (`http://`, `https://`) or a path; `file://` links and the quotes a file
    /// manager adds when copying a path are accepted. `None` when nothing was typed.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().trim_matches(['"', '\'']).trim();
        if text.is_empty() {
            return None;
        }
        let lower = text.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            return Some(Self::Url(text.to_string()));
        }
        let path = if lower.starts_with("file://") {
            let rest = &text["file://".len()..];
            // `file:///C:/x.png` on Windows: the slash before the drive letter is not part.
            let rest = match rest.as_bytes() {
                [b'/', _, b':', ..] => &rest[1..],
                _ => rest,
            };
            percent_decode(rest)
        } else {
            text.to_string()
        };
        Some(Self::File(PathBuf::from(path)))
    }

    /// The file name the source suggests, without extension; may be empty.
    fn stem(&self) -> String {
        let last = match self {
            Self::Url(url) => {
                let path = url.split(['?', '#']).next().unwrap_or_default();
                let path = path.split_once("://").map_or(path, |(_, rest)| rest);
                // Only the part after the host names a file.
                match path.split_once('/') {
                    Some((_, rest)) => percent_decode(rest.rsplit('/').next().unwrap_or_default()),
                    None => String::new(),
                }
            }
            Self::File(path) => path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        };
        match last.rsplit_once('.') {
            Some((stem, _)) => stem.to_string(),
            None => last,
        }
    }

    /// The image's bytes: read from disk or downloaded.
    fn read(&self) -> Result<Vec<u8>> {
        match self {
            Self::File(path) => {
                let meta = fs::metadata(path)
                    .with_context(|| format!("there is no file at {}", path.display()))?;
                if !meta.is_file() {
                    bail!("{} is not a file", path.display());
                }
                if meta.len() > MAX_IMAGE_BYTES {
                    bail!("the image is larger than {} MB", MAX_IMAGE_BYTES / 1024 / 1024);
                }
                fs::read(path).with_context(|| format!("reading {}", path.display()))
            }
            Self::Url(url) => ureq::get(url)
                .call()
                .with_context(|| format!("downloading {url}"))?
                .body_mut()
                .with_config()
                .limit(MAX_IMAGE_BYTES)
                .read_to_vec()
                .with_context(|| format!("downloading {url}")),
        }
    }
}

/// Copies the image of `source` into `dir` (created if missing) and returns the new sleeve.
/// `name` is the sleeve's name without extension; empty, the source's file name is used.
/// A taken name gets a ` (2)`, ` (3)`… suffix.
pub fn add_sleeve(dir: &Path, source: &SleeveSource, name: &str) -> Result<Sleeve> {
    let bytes = source.read()?;
    let extension = image_extension(&bytes)?;
    let name = if name.trim().is_empty() {
        source.stem()
    } else {
        name.to_string()
    };
    let mut stem = file_stem_for(&name);
    // `file_stem_for` names nameless decks; a sleeve gets a name of its own.
    if name.trim().trim_matches('.').trim().is_empty() {
        stem = FALLBACK_NAME.to_string();
    }
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = free_path(dir, &stem, extension);
    fs::write(&path, &bytes).with_context(|| format!("writing {}", path.display()))?;
    Ok(Sleeve {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| anyhow!("the sleeve has no file name"))?,
        path,
    })
}

/// `jpg` or `png` for an image the simulator can show; an error for anything else.
fn image_extension(bytes: &[u8]) -> Result<&'static str> {
    let format = image::guess_format(bytes).map_err(|_| {
        anyhow!("this is not an image (a web page instead of the picture itself?)")
    })?;
    let extension = match format {
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Png => "png",
        other => bail!(
            "{} images are not supported: use a JPEG or PNG",
            other.extensions_str().first().unwrap_or(&"these").to_uppercase()
        ),
    };
    image::load_from_memory_with_format(bytes, format)
        .context("the image is damaged and cannot be read")?;
    Ok(extension)
}

/// `dir/stem.extension`, or `dir/stem (2).extension`… when a sleeve already has that name
/// (whatever its extension or case: the decks name sleeves case-insensitively).
fn free_path(dir: &Path, stem: &str, extension: &str) -> PathBuf {
    let taken: Vec<String> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| {
                    let path = e.path();
                    Some(path.file_stem()?.to_string_lossy().to_lowercase())
                })
                .collect()
        })
        .unwrap_or_default();
    let mut candidate = stem.to_string();
    let mut n = 2;
    while taken.contains(&candidate.to_lowercase()) {
        candidate = format!("{stem} ({n})");
        n += 1;
    }
    dir.join(format!("{candidate}.{extension}"))
}

/// `%20` and the like decoded; invalid escapes are kept as they are.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) = text
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png() -> Vec<u8> {
        let mut bytes = Vec::new();
        image::RgbImage::new(2, 3)
            .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn sources() {
        assert_eq!(SleeveSource::parse("  "), None);
        assert_eq!(
            SleeveSource::parse(" https://x.org/a.png "),
            Some(SleeveSource::Url("https://x.org/a.png".into()))
        );
        assert_eq!(
            SleeveSource::parse("\"/home/me/my sleeve.png\""),
            Some(SleeveSource::File("/home/me/my sleeve.png".into()))
        );
        assert_eq!(
            SleeveSource::parse("file:///home/me/my%20sleeve.png"),
            Some(SleeveSource::File("/home/me/my sleeve.png".into()))
        );
        assert_eq!(
            SleeveSource::parse("file:///C:/Games/x.png"),
            Some(SleeveSource::File("C:/Games/x.png".into()))
        );
    }

    #[test]
    fn stems() {
        let url = |u: &str| SleeveSource::Url(u.into()).stem();
        assert_eq!(url("https://x.org/img/RZ%20Rem.jpg?w=200#top"), "RZ Rem");
        assert_eq!(url("https://x.org"), "");
        assert_eq!(url("https://x.org/"), "");
        assert_eq!(SleeveSource::File("/a/b/Emilia.png".into()).stem(), "Emilia");
    }

    #[test]
    fn adds_a_sleeve_from_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let source_path = dir.path().join("Rem.png");
        fs::write(&source_path, png()).unwrap();
        let sleeves = dir.path().join("Sleeves");
        let source = SleeveSource::File(source_path);

        let first = add_sleeve(&sleeves, &source, "").unwrap();
        assert_eq!(first.name, "Rem.png");
        assert_eq!(fs::read(&first.path).unwrap(), png());

        // Never overwrites, whatever the case of the taken name.
        fs::write(sleeves.join("rem (2).JPG"), "").unwrap();
        assert_eq!(add_sleeve(&sleeves, &source, "").unwrap().name, "Rem (3).png");

        assert_eq!(
            add_sleeve(&sleeves, &source, "Re:Zero / Rem").unwrap().name,
            "Re-Zero - Rem.png"
        );
        assert_eq!(add_sleeve(&sleeves, &source, " . ").unwrap().name, "Sleeve.png");
    }

    #[test]
    fn refuses_what_is_not_a_usable_image() {
        let dir = tempfile::tempdir().unwrap();
        let sleeves = dir.path().join("Sleeves");

        let missing = SleeveSource::File(dir.path().join("nope.png"));
        assert!(add_sleeve(&sleeves, &missing, "").is_err());

        let html = dir.path().join("page.png");
        fs::write(&html, "<html></html>").unwrap();
        let err = add_sleeve(&sleeves, &SleeveSource::File(html), "").unwrap_err();
        assert!(err.to_string().contains("not an image"));

        let gif = dir.path().join("a.gif");
        fs::write(&gif, b"GIF89a\x01\x00\x01\x00\x00\x00\x00;").unwrap();
        let err = add_sleeve(&sleeves, &SleeveSource::File(gif), "").unwrap_err();
        assert!(err.to_string().contains("GIF"), "{err}");

        let truncated = dir.path().join("cut.png");
        fs::write(&truncated, &png()[..20]).unwrap();
        assert!(add_sleeve(&sleeves, &SleeveSource::File(truncated), "").is_err());

        // Nothing was written.
        assert!(crate::decks::list_sleeves(&sleeves).is_empty());
    }
}
