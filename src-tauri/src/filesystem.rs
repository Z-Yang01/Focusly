//! 文件系统服务：数据目录布局、启动备份、结构化错误日志（error/*.md）。

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::AppResult;
use tauri::Manager;

pub struct AppPaths {
    pub root: PathBuf,
    pub db: PathBuf,
    pub images: PathBuf,
    pub backups: PathBuf,
    pub logs: PathBuf,
    pub errors: PathBuf,
}

/// 便携模式：exe 同目录存在 `portable.marker` 文件时，
/// 全部数据（数据库/图片/备份/日志）写入 `<exe目录>/data/`，随程序走、不落 %APPDATA%。
/// 否则回退系统应用数据目录。
pub fn resolve_data_root(app: &tauri::AppHandle) -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if let Some(root) = portable_root(dir) {
                log::info!("便携模式启用，数据目录: {}", root.display());
                return root;
            }
        }
    }
    let _ = app;
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("com.focusly.app"))
}

/// 纯函数：给定 exe 所在目录，返回便携数据根（None = 非便携）。
pub fn portable_root(exe_dir: &Path) -> Option<PathBuf> {
    if exe_dir.join("portable.marker").is_file() {
        Some(exe_dir.join("data"))
    } else {
        None
    }
}

impl AppPaths {
    pub fn init(root: PathBuf) -> AppResult<Self> {
        let paths = Self {
            db: root.join("database.sqlite"),
            images: root.join("images"),
            backups: root.join("backups"),
            logs: root.join("logs"),
            errors: root.join("errors"),
            root,
        };
        for dir in [&paths.images, &paths.backups, &paths.logs, &paths.errors] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(paths)
    }
}

/// 启动时轮转备份：database.sqlite → backups/，最多保留 5 份。
/// 数据库不存在（首次启动）则跳过。
pub fn backup_database(paths: &AppPaths) -> AppResult<()> {
    if !paths.db.exists() {
        return Ok(());
    }
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let dest = paths.backups.join(format!("database-{stamp}.sqlite"));
    std::fs::copy(&paths.db, &dest)?;

    // WAL/SHM 一并拷贝，保证备份点一致（WAL checkpoint 在下次打开时自然回收）
    for ext in ["-wal", "-shm"] {
        let src = PathBuf::from(format!("{}{}", paths.db.display(), ext));
        if src.exists() {
            let _ = std::fs::copy(
                &src,
                paths.backups.join(format!("database-{stamp}.sqlite{ext}")),
            );
        }
    }

    // 清理旧备份，保留最近 5 份
    let mut backups: Vec<PathBuf> = std::fs::read_dir(&paths.backups)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("database-") && n.ends_with(".sqlite"))
                .unwrap_or(false)
        })
        .collect();
    backups.sort();
    while backups.len() > 5 {
        let oldest = backups.remove(0);
        let _ = std::fs::remove_file(&oldest);
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-wal", oldest.display())));
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-shm", oldest.display())));
    }
    log::info!("数据库备份完成: {}", dest.display());
    Ok(())
}

/// 结构化错误日志：errors/YYYY-MM-DD-<category>.md
/// 记录：问题 / 原因 / 影响 / 解决方式。
pub fn journal_error(
    paths: &AppPaths,
    category: &str,
    problem: &str,
    cause: &str,
    impact: &str,
    solution: &str,
) {
    let file = paths.errors.join(format!(
        "{}-{}.md",
        chrono::Local::now().format("%Y-%m-%d"),
        sanitize(category)
    ));
    let entry = format!(
        "\n## {} — {problem}\n\n- **问题**: {problem}\n- **原因**: {cause}\n- **影响**: {impact}\n- **解决方式**: {solution}\n",
        chrono::Local::now().format("%H:%M:%S"),
    );
    let appended = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .and_then(|mut f| {
            if f.metadata().map(|m| m.len() == 0).unwrap_or(true) {
                use std::io::Write;
                writeln!(f, "# Focusly 错误日志 — {} ({category})\n", chrono::Local::now().format("%Y-%m-%d"))?;
            }
            f.write_all(entry.as_bytes())
        });
    if let Err(e) = appended {
        log::error!("错误日志写入失败: {e}（原始错误: {problem}）");
    }
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// 确保数据目录内的路径是安全的（不允许逃逸出 images 根目录）。
pub fn is_inside_root(root: &Path, p: &Path) -> bool {
    p.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_backup_rotation() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::init(tmp.path().to_path_buf()).unwrap();
        assert!(paths.images.is_dir() && paths.backups.is_dir());

        // 无数据库 → 跳过
        backup_database(&paths).unwrap();

        std::fs::write(&paths.db, b"fake").unwrap();
        for i in 0..7 {
            std::thread::sleep(std::time::Duration::from_millis(1100));
            let _ = i;
            std::fs::write(&paths.db, format!("v{i}")).unwrap();
            backup_database(&paths).unwrap();
        }
        let count = std::fs::read_dir(&paths.backups)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("database-")
            })
            .count();
        assert!(count <= 5, "备份应轮转，实际 {count} 份");
    }

    #[test]
    fn error_journal_writes() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::init(tmp.path().to_path_buf()).unwrap();
        journal_error(&paths, "database", "问题A", "原因B", "影响C", "方案D");
        let files: Vec<_> = std::fs::read_dir(&paths.errors)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        assert_eq!(files.len(), 1);
        let content = std::fs::read_to_string(&files[0]).unwrap();
        assert!(content.contains("问题A") && content.contains("方案D"));
    }

    #[test]
    fn portable_root_detection() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(portable_root(tmp.path()).is_none(), "无 marker 非便携");
        std::fs::write(tmp.path().join("portable.marker"), b"").unwrap();
        let root = portable_root(tmp.path()).unwrap();
        assert_eq!(root, tmp.path().join("data"));
    }

    #[test]
    fn inside_root_check() {
        let root = Path::new("C:/data/Focusly/images");
        assert!(is_inside_root(root, Path::new("C:/data/Focusly/images/n1/a.png")));
        assert!(!is_inside_root(root, Path::new("C:/Windows/system32/a.png")));
    }
}
