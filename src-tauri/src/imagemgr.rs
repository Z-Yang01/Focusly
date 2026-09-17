//! 图片管理增强：重复检测（SHA-256 内容哈希）、孤儿文件清理、缩略图生成。
//!
//! # 未来命令（由总控接线后暴露给前端，参数均为无参）
//! - `image_find_duplicates() -> Vec<DupGroup>`：遍历 note_images 全表，对存在的
//!   文件按 SHA-256 内容哈希分组，仅返回 ≥2 条的组（文件缺失跳过并记日志）。
//! - `image_cleanup_orphans() -> usize`：扫描 paths.images 下所有文件，删除不在
//!   note_images.path 集合中的文件并返回数量（仅删 images 根目录内文件，防误删）。
//! - `image_make_thumbnails() -> usize`：为每张图片生成 `<data>/thumbs/<image_id>.jpg`
//!   （最长边 320 等比缩放，JPEG 质量 80），返回本次生成数量；原图损坏/缺失跳过。
//!
//! # 接线点（本模块不改动其他文件，由总控执行）
//! - `lib.rs`：模块声明 `mod imagemgr;`
//! - `commands/`：新建薄命令层（参照 images_cmd.rs，`State<AppState>` → 本模块三个 pub fn），
//!   并在 `generate_handler!` 注册上述三个命令名。
//!
//! 说明：note_images 的读取已收敛进 DAO（db::images::list_all），本文件不再持 SQL。

use image::GenericImageView;
use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::Manager;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// 缩略图最长边（等比缩放，不放大）。
const THUMB_MAX_EDGE: u32 = 320;
/// 缩略图 JPEG 编码质量。
const THUMB_JPEG_QUALITY: u8 = 80;

/// 一条重复图片记录（serde camelCase，对应前端 DupEntry）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DupEntry {
    pub image_id: String,
    pub note_id: String,
    pub path: String,
    pub filename: String,
    pub size: u64,
}

/// 一个重复组：同一 SHA-256 内容哈希对应 ≥2 个文件（对应前端 DupGroup）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DupGroup {
    pub hash: String,
    pub entries: Vec<DupEntry>,
}

/// 计算文件内容的 SHA-256（hex 小写）。便签图片量级下整体读入即可。
pub fn file_sha256(path: &Path) -> AppResult<String> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(to_hex(&hasher.finalize()))
}

fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// note_images 行的轻量投影（来自 DAO db::images::list_all）。
struct ImageRow {
    id: String,
    note_id: String,
    path: String,
    filename: String,
}

/// 读取全表行（先取数后做文件 IO，避免长时间占用数据库锁）。
fn load_rows(app: &tauri::AppHandle) -> AppResult<Vec<ImageRow>> {
    let state = app.state::<AppState>();
    let rows = state.db.with(crate::db::images::list_all)?;
    Ok(rows
        .into_iter()
        .map(|img| ImageRow {
            id: img.id,
            note_id: img.note_id,
            path: img.path,
            filename: img.filename,
        })
        .collect())
}

/// 按文件内容 SHA-256 找重复图片：仅返回 ≥2 条的组；缺失/不可读文件跳过并记日志。
pub fn find_duplicates(app: &tauri::AppHandle) -> AppResult<Vec<DupGroup>> {
    let rows = load_rows(app)?;
    // BTreeMap 保证输出按哈希稳定排序
    let mut groups: BTreeMap<String, Vec<DupEntry>> = BTreeMap::new();
    for row in rows {
        let p = Path::new(&row.path);
        if !p.is_file() {
            log::warn!("重复检测跳过缺失文件: {} (image {})", row.path, row.id);
            continue;
        }
        let hash = match file_sha256(p) {
            Ok(h) => h,
            Err(e) => {
                log::warn!("重复检测读取哈希失败，跳过 {}: {e}", row.path);
                continue;
            }
        };
        let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        groups.entry(hash).or_default().push(DupEntry {
            image_id: row.id,
            note_id: row.note_id,
            path: row.path,
            filename: row.filename,
            size,
        });
    }
    Ok(groups
        .into_iter()
        .filter(|(_, entries)| entries.len() >= 2)
        .map(|(hash, entries)| DupGroup { hash, entries })
        .collect())
}

/// 纯函数：路径未被数据库引用即为孤儿。
pub fn is_orphan(path: &str, referenced: &HashSet<String>) -> bool {
    !referenced.contains(path)
}

/// 清理 images/ 下的孤儿文件：扫描目录内所有文件，凡不在 note_images.path
/// 集合中的直接删除，返回删除数量。防误删：删除前用 filesystem::is_inside_root
/// 复查路径必须位于 images 根目录之内（子目录递归扫描的兜底）。
pub fn cleanup_orphans(app: &tauri::AppHandle) -> AppResult<usize> {
    let state = app.state::<AppState>();
    let images_root = state.paths.images.clone();
    let referenced: HashSet<String> = load_rows(app)?.into_iter().map(|r| r.path).collect();

    let mut removed = 0usize;
    for file in collect_files(&images_root)? {
        if !crate::filesystem::is_inside_root(&images_root, &file) {
            log::warn!("孤儿清理跳过 images 根目录之外的路径: {}", file.display());
            continue;
        }
        let path_str = file.to_string_lossy().into_owned();
        if !is_orphan(&path_str, &referenced) {
            continue;
        }
        match std::fs::remove_file(&file) {
            Ok(()) => {
                removed += 1;
                log::info!("已删除孤儿图片文件: {}", file.display());
            }
            Err(e) => log::warn!("孤儿文件删除失败 {}: {e}", file.display()),
        }
    }
    log::info!(
        "孤儿清理完成：删除 {removed} 个文件（images 根目录 {}）",
        images_root.display()
    );
    Ok(removed)
}

/// 递归收集目录下的所有普通文件（不引入 walkdir 依赖）。
fn collect_files(root: &Path) -> AppResult<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.is_file() {
                out.push(p);
            }
        }
    }
    Ok(out)
}

/// 为全库图片生成缩略图：`<images 的兄弟目录>/thumbs/<image_id>.jpg`。
/// 已存在的跳过；原图损坏/缺失记日志跳过（不计入返回值）。返回本次生成数量。
pub fn make_thumbnails(app: &tauri::AppHandle) -> AppResult<usize> {
    let state = app.state::<AppState>();
    // thumbs 是 images 的兄弟目录：root/images → root/thumbs
    let thumbs_dir = state
        .paths
        .images
        .parent()
        .ok_or_else(|| AppError::Io("images 目录缺少父目录，无法确定 thumbs 位置".into()))?
        .join("thumbs");
    std::fs::create_dir_all(&thumbs_dir)?;

    let rows = load_rows(app)?;
    let mut generated = 0usize;
    for row in rows {
        let thumb_path = thumbs_dir.join(format!("{}.jpg", row.id));
        if thumb_path.is_file() {
            continue;
        }
        match make_one_thumbnail(Path::new(&row.path), &thumb_path) {
            Ok(()) => generated += 1,
            Err(e) => log::warn!("缩略图跳过 {} (image {}): {e}", row.path, row.id),
        }
    }
    log::info!(
        "缩略图生成完成：本次生成 {generated} 张 → {}",
        thumbs_dir.display()
    );
    Ok(generated)
}

/// 单张缩略图：最长边 320 等比缩放（Triangle），JPEG 质量 80。
/// 已 ≤320 的图按自身尺寸 resize（不放大）；损坏/缺失原图在此报错，由调用方跳过。
fn make_one_thumbnail(src: &Path, dest: &Path) -> AppResult<()> {
    let img = image::open(src)?;
    let (w, h) = img.dimensions();
    let longest = w.max(h).max(1);
    let (nw, nh) = if longest <= THUMB_MAX_EDGE {
        (w.max(1), h.max(1))
    } else {
        let f = THUMB_MAX_EDGE as f32 / longest as f32;
        (
            (((w as f32) * f).round() as u32).max(1),
            (((h as f32) * f).round() as u32).max(1),
        )
    };
    let resized = image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle);
    let rgb = image::DynamicImage::ImageRgba8(resized).to_rgb8();

    use image::ImageEncoder;
    let file = std::fs::File::create(dest)?;
    let mut writer = std::io::BufWriter::new(file);
    let encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, THUMB_JPEG_QUALITY);
    let (tw, th) = rgb.dimensions();
    encoder.write_image(rgb.as_raw(), tw, th, image::ExtendedColorType::Rgb8)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vectors() {
        let dir = tempfile::tempdir().unwrap();

        // 空文件 → 空串已知向量
        let empty = dir.path().join("empty.bin");
        std::fs::write(&empty, b"").unwrap();
        assert_eq!(
            file_sha256(&empty).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        // "abc" → FIPS 180-2 标准向量
        let abc = dir.path().join("abc.bin");
        std::fs::write(&abc, b"abc").unwrap();
        assert_eq!(
            file_sha256(&abc).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn dup_group_serializes_camel_case() {
        let group = DupGroup {
            hash: "abc123".into(),
            entries: vec![DupEntry {
                image_id: "img-1".into(),
                note_id: "note-1".into(),
                path: "C:/data/Focusly/images/note-1/a.png".into(),
                filename: "a.png".into(),
                size: 1024,
            }],
        };
        let json = serde_json::to_string(&group).unwrap();
        assert!(json.contains("\"hash\":\"abc123\""), "{json}");
        assert!(json.contains("\"imageId\":\"img-1\""), "{json}");
        assert!(json.contains("\"noteId\":\"note-1\""), "{json}");
        assert!(json.contains("\"size\":1024"), "{json}");
        assert!(!json.contains("image_id"), "禁止 snake_case 输出: {json}");
    }

    #[test]
    fn orphan_detection() {
        let mut referenced = HashSet::new();
        referenced.insert("C:/data/Focusly/images/n1/a.png".to_string());
        // 被引用 → 非孤儿
        assert!(!is_orphan("C:/data/Focusly/images/n1/a.png", &referenced));
        // 未被引用 → 孤儿
        assert!(is_orphan("C:/data/Focusly/images/n1/b.png", &referenced));
        // 空引用集合 → 全部为孤儿
        assert!(is_orphan(
            "C:/data/Focusly/images/n1/a.png",
            &HashSet::new()
        ));
    }
}
