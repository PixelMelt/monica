//! Local file storage (X020), replacing Uploadcare: the bytes of every
//! `files` row live at `<DATA_DIR>/files/<vault_id>/<uuid>`, and the
//! 300×300 thumbnail of an image next to it as `<uuid>.thumb` (made on first
//! request). Services only write and delete rows; [`Storage::sweep`] deletes
//! bytes no row points at (E009, `DeleteFileInStorage`).

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::ImageFormat;
use image::imageops::FilterType;
use sea_orm::{ConnectionTrait, EntityTrait};

use crate::db::uuid::Uuid;
use crate::entities::files;

pub const THUMBNAIL_SIZE: u32 = 300;

/// Image types accepted for photos and avatars.
pub const IMAGE_TYPES: [&str; 4] = ["image/jpeg", "image/png", "image/gif", "image/webp"];

#[derive(Clone, Debug)]
pub struct Storage {
    root: PathBuf,
}

impl Storage {
    pub fn new(data_dir: &Path) -> Storage {
        Storage { root: data_dir.join("files") }
    }

    fn dir(&self, vault_id: Uuid) -> PathBuf {
        self.root.join(vault_id.to_string())
    }

    /// Where the bytes of `uuid` are.
    pub fn path(&self, vault_id: Uuid, uuid: &str) -> PathBuf {
        self.dir(vault_id).join(sanitize(uuid))
    }

    fn thumbnail_path(&self, vault_id: Uuid, uuid: &str) -> PathBuf {
        self.dir(vault_id).join(format!("{}.thumb", sanitize(uuid)))
    }

    /// Stores `bytes` and returns the new storage key (a uuid).
    pub async fn put(&self, vault_id: Uuid, bytes: &[u8]) -> std::io::Result<String> {
        let key = Uuid::new().to_string();
        let dir = self.dir(vault_id);
        tokio::fs::create_dir_all(&dir).await?;
        let tmp = dir.join(format!(".{key}.part"));
        tokio::fs::write(&tmp, bytes).await?;
        tokio::fs::rename(&tmp, dir.join(&key)).await?;
        Ok(key)
    }

    /// The bytes of `uuid`.
    pub async fn get(&self, vault_id: Uuid, uuid: &str) -> std::io::Result<Vec<u8>> {
        tokio::fs::read(self.path(vault_id, uuid)).await
    }

    /// The 300×300 centre crop of an image, as JPEG (PNG when the image has
    /// transparency), made once and cached.
    pub async fn thumbnail(&self, vault_id: Uuid, uuid: &str) -> std::io::Result<(Vec<u8>, &'static str)> {
        let cached = self.thumbnail_path(vault_id, uuid);
        if let Ok(bytes) = tokio::fs::read(&cached).await {
            return Ok((bytes.clone(), thumbnail_type(&bytes)));
        }
        let original = self.get(vault_id, uuid).await?;
        let bytes =
            tokio::task::spawn_blocking(move || make_thumbnail(&original)).await.map_err(std::io::Error::other)??;
        let tmp = cached.with_extension("thumb.part");
        tokio::fs::write(&tmp, &bytes).await?;
        tokio::fs::rename(&tmp, &cached).await?;
        Ok((bytes.clone(), thumbnail_type(&bytes)))
    }

    /// Deletes the bytes (and thumbnail) of `uuid`; missing files are fine.
    pub async fn delete(&self, vault_id: Uuid, uuid: &str) {
        let _ = tokio::fs::remove_file(self.path(vault_id, uuid)).await;
        let _ = tokio::fs::remove_file(self.thumbnail_path(vault_id, uuid)).await;
    }

    /// Deletes everything stored for `vault_id` (a deleted vault or account).
    pub async fn delete_vault(&self, vault_id: Uuid) {
        let _ = tokio::fs::remove_dir_all(self.dir(vault_id)).await;
    }

    /// Deletes stored bytes that no `files` row points at, and the
    /// directories of vaults that are gone. Returns how many files went.
    pub async fn sweep<C: ConnectionTrait>(&self, db: &C) -> std::io::Result<usize> {
        let rows = files::Entity::find().all(db).await.map_err(std::io::Error::other)?;
        let keep: std::collections::HashSet<(String, String)> =
            rows.iter().map(|f| (f.vault_id.to_string(), sanitize(&f.uuid))).collect();
        let mut removed = 0;
        let Ok(mut vaults) = tokio::fs::read_dir(&self.root).await else { return Ok(0) };
        while let Some(vault) = vaults.next_entry().await? {
            let vault_name = vault.file_name().to_string_lossy().into_owned();
            let mut entries = tokio::fs::read_dir(vault.path()).await?;
            while let Some(entry) = entries.next_entry().await? {
                let name = entry.file_name().to_string_lossy().into_owned();
                let key = name.strip_suffix(".thumb").unwrap_or(&name).to_owned();
                if name.starts_with('.') || !keep.contains(&(vault_name.clone(), key)) {
                    tokio::fs::remove_file(entry.path()).await?;
                    removed += 1;
                }
            }
            let _ = tokio::fs::remove_dir(vault.path()).await;
        }
        Ok(removed)
    }
}

/// Keeps a storage key to one path segment.
fn sanitize(uuid: &str) -> String {
    uuid.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect()
}

fn thumbnail_type(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") { "image/png" } else { "image/jpeg" }
}

fn make_thumbnail(original: &[u8]) -> std::io::Result<Vec<u8>> {
    let img = image::load_from_memory(original).map_err(std::io::Error::other)?;
    let thumb = img.resize_to_fill(THUMBNAIL_SIZE, THUMBNAIL_SIZE, FilterType::Lanczos3);
    let mut out = Cursor::new(Vec::new());
    if thumb.color().has_alpha() {
        thumb.write_to(&mut out, ImageFormat::Png).map_err(std::io::Error::other)?;
    } else {
        thumb.into_rgb8().write_to(&mut out, ImageFormat::Jpeg).map_err(std::io::Error::other)?;
    }
    Ok(out.into_inner())
}

/// The media type of uploaded bytes: sniffed from the content, else from
/// the file name, else the type the client sent.
pub fn media_type(bytes: &[u8], file_name: &str, declared: Option<&str>) -> String {
    if let Some(t) = infer::get(bytes) {
        return t.mime_type().to_owned();
    }
    if let Some(t) = mime_guess::from_path(file_name).first() {
        return t.essence_str().to_owned();
    }
    declared.unwrap_or("application/octet-stream").to_owned()
}

/// Whether `bytes` decode as one of [`IMAGE_TYPES`].
pub fn is_image(bytes: &[u8]) -> bool {
    infer::get(bytes).is_some_and(|t| IMAGE_TYPES.contains(&t.mime_type())) && image::load_from_memory(bytes).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 255) as u8, (y % 255) as u8, 128]));
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[tokio::test]
    async fn stores_thumbnails_and_sweeps() {
        let dir = std::env::temp_dir().join(format!("monica_storage_{}", Uuid::new()));
        let s = Storage::new(&dir);
        let vault = Uuid::new();
        let key = s.put(vault, &png(800, 400)).await.unwrap();
        assert!(is_image(&s.get(vault, &key).await.unwrap()));
        let (thumb, ty) = s.thumbnail(vault, &key).await.unwrap();
        assert_eq!(ty, "image/jpeg");
        let t = image::load_from_memory(&thumb).unwrap();
        assert_eq!((t.width(), t.height()), (300, 300));
        assert_eq!(media_type(&png(2, 2), "x.bin", None), "image/png");
        assert_eq!(media_type(b"%PDF-1.4 hello", "x.pdf", None), "application/pdf");
        assert!(!is_image(b"not an image"));
        s.delete(vault, &key).await;
        assert!(s.get(vault, &key).await.is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
