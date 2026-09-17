use std::path::Path;

use rusqlite::{params, Connection};
use uuid::Uuid;

use super::models::NoteImage;
use crate::error::{AppError, AppResult};

pub const ALLOWED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn row_to_image(r: &Row) -> rusqlite::Result<NoteImage> {
    Ok(NoteImage {
        id: r.get("id")?,
        note_id: r.get("note_id")?,
        path: r.get("path")?,
        filename: r.get("filename")?,
        width: r.get("width")?,
        height: r.get("height")?,
        created_at: r.get("created_at")?,
    })
}

use rusqlite::Row;

pub fn add_file(
    conn: &Connection,
    images_root: &Path,
    note_id: &str,
    src_path: &Path,
) -> AppResult<NoteImage> {
    let ext = src_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .ok_or_else(|| AppError::Invalid("图片文件缺少扩展名".into()))?;
    if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::Invalid(format!(
            "不支持的图片格式 .{ext}（仅支持 {}）",
            ALLOWED_EXTENSIONS.join(", ")
        )));
    }
    let filename = format!("{}.{}", Uuid::new_v4(), ext);
    let dest_dir = images_root.join(note_id);
    std::fs::create_dir_all(&dest_dir)?;
    let dest = dest_dir.join(&filename);
    std::fs::copy(src_path, &dest)?;

    let (w, h) = read_dimensions(&dest)?;
    insert(conn, note_id, &dest.to_string_lossy(), &filename, w, h)
}

/// 从剪贴板粘贴的图片：前端传入 RGBA 原始字节，编码为 PNG 存储。
pub fn add_rgba(
    conn: &Connection,
    images_root: &Path,
    note_id: &str,
    rgba: &[u8],
    width: u32,
    height: u32,
) -> AppResult<NoteImage> {
    if rgba.len() != (width as usize) * (height as usize) * 4 {
        return Err(AppError::Invalid("图片数据大小与尺寸不匹配".into()));
    }
    let filename = format!("{}.png", Uuid::new_v4());
    let dest_dir = images_root.join(note_id);
    std::fs::create_dir_all(&dest_dir)?;
    let dest = dest_dir.join(&filename);
    image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| AppError::Image("无法解码 RGBA 数据".into()))?
        .save_with_format(&dest, image::ImageFormat::Png)?;

    insert(
        conn,
        note_id,
        &dest.to_string_lossy(),
        &filename,
        Some(width as i32),
        Some(height as i32),
    )
}

fn insert(
    conn: &Connection,
    note_id: &str,
    path: &str,
    filename: &str,
    width: Option<i32>,
    height: Option<i32>,
) -> AppResult<NoteImage> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO note_images (id, note_id, path, filename, width, height, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, note_id, path, filename, width, height, now()],
    )?;
    get(conn, &id)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<NoteImage> {
    conn.query_row(
        "SELECT id, note_id, path, filename, width, height, created_at \
         FROM note_images WHERE id=?1",
        params![id],
        row_to_image,
    )
    .map_err(|e| AppError::Db(e.to_string()))
}

pub fn list_for_note(conn: &Connection, note_id: &str) -> AppResult<Vec<NoteImage>> {
    let mut stmt = conn.prepare(
        "SELECT id, note_id, path, filename, width, height, created_at \
         FROM note_images WHERE note_id=?1 ORDER BY created_at ASC",
    )?;
    let list = stmt
        .query_map(params![note_id], row_to_image)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(list)
}

/// 全表图片记录（imagemgr 重复检测/孤儿清理/缩略图用；调用方先取数后做文件 IO）。
pub fn list_all(conn: &Connection) -> AppResult<Vec<NoteImage>> {
    let mut stmt = conn.prepare(
        "SELECT id, note_id, path, filename, width, height, created_at \
         FROM note_images",
    )?;
    let list = stmt
        .query_map([], row_to_image)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(list)
}

pub fn remove(conn: &Connection, images_root: &Path, id: &str) -> AppResult<()> {
    let img = get(conn, id)?;
    conn.execute("DELETE FROM note_images WHERE id=?1", params![id])?;
    delete_file_best_effort(images_root, &img.path);
    Ok(())
}

pub fn delete_file_best_effort(images_root: &Path, path: &str) {
    let p = Path::new(path);
    // 仅清理应用数据目录内的文件，防止误删用户任意路径
    if p.starts_with(images_root) {
        let _ = std::fs::remove_file(p);
    }
}

/// 删除便签图片目录（随永久删除调用；归档不调用）。
pub fn cleanup_note_dir(images_root: &Path, note_id: &str) {
    let dir = images_root.join(note_id);
    if dir.exists() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

fn read_dimensions(path: &Path) -> AppResult<(Option<i32>, Option<i32>)> {
    match image::image_dimensions(path) {
        Ok((w, h)) => Ok((Some(w as i32), Some(h as i32))),
        Err(e) => {
            log::warn!("读取图片尺寸失败 {}: {e}", path.display());
            Ok((None, None))
        }
    }
}

/// 检测图片路径是否仍存在（前端加载失败时调用）。
pub fn path_exists(path: &str) -> bool {
    Path::new(path).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Connection, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrations::run(&conn).unwrap();
        (conn, dir)
    }

    #[test]
    fn add_rgba_and_cleanup() {
        let (conn, dir) = setup();
        let n = super::super::notes::create(&conn, "t", "c").unwrap();
        let img = add_rgba(&conn, dir.path(), &n.id, &[255, 0, 0, 255], 1, 1).unwrap();
        assert_eq!(img.width, Some(1));
        assert!(Path::new(&img.path).is_file());
        assert!(img.path.contains(&n.id), "图片应存储在 note 子目录");

        let list = list_for_note(&conn, &n.id).unwrap();
        assert_eq!(list.len(), 1);

        cleanup_note_dir(dir.path(), &n.id);
        assert!(!Path::new(&img.path).exists(), "清理后文件应删除");
        assert!(!path_exists(&img.path));
    }

    #[test]
    fn rejects_unsupported_extension() {
        let (conn, dir) = setup();
        let n = super::super::notes::create(&conn, "t", "c").unwrap();
        let src = dir.path().join("evil.exe");
        std::fs::write(&src, b"MZ").unwrap();
        let r = add_file(&conn, dir.path(), &n.id, &src);
        assert!(r.is_err());
    }
}
