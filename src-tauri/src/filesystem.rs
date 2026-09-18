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

/// 数据目录解析策略（按优先级）：
/// 1. NSIS 安装模式：exe 所在目录下存在 `data/` 目录（安装器自动创建）→ 数据写在该目录
/// 2. 便携模式：exe 旁有 `portable.marker` → 数据写在 `<exe目录>/data/`
/// 3. 开发模式（target/debug 或 target/release）→ 回退 %APPDATA%
/// 4. 兜底：%APPDATA%
/// 4. 兜底：%APPDATA%
///
/// 这样安装到哪、数据就在哪，卸载重装数据不丢。
pub fn resolve_data_root(app: &tauri::AppHandle) -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // 开发环境排除：target/debug 或 target/release 下的 exe 不算已安装
            let is_dev = dir
                .to_str()
                .map(|d| d.contains("target") && (d.contains("debug") || d.contains("release")))
                .unwrap_or(false);

            // 条件 1 或 2：exe 旁有 data/ 目录或 portable.marker → 本地数据模式
            if is_local_data_mode(dir) && !is_dev {
                log::info!("本地数据模式，数据目录: {}", dir.join("data").display());
                return dir.join("data");
            }
        }
    }
    // 开发模式或未安装 → %APPDATA%
    let _ = app;
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("com.focusly.app"))
}

/// 纯函数：判断是否为本地数据模式。
pub fn is_local_data_mode(exe_dir: &Path) -> bool {
    exe_dir.join("data").is_dir() || exe_dir.join("portable.marker").is_file()
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

/// 启动时轮转备份：通过 SQLite Online Backup API 生成原子一致性快照，
/// 最多保留 5 份。数据库不存在（首次启动）则跳过。
/// 注意：必须在拿到 `&Db` 连接后调用（备份 API 需要源连接）。
pub fn backup_database(db: &crate::db::Db, paths: &AppPaths) -> AppResult<()> {
    use rusqlite::backup::Backup;
    use std::time::Duration;

    if !paths.db.exists() {
        return Ok(());
    }
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let dest = paths.backups.join(format!("database-{stamp}.sqlite"));

    // Online Backup API：把源库（含未合并的 WAL 页）以页为单位复制成一致快照
    db.with(|src| -> AppResult<()> {
        let mut dst = rusqlite::Connection::open(&dest)?;
        {
            let backup = Backup::new(src, &mut dst)?;
            backup.run_to_completion(512, Duration::from_millis(5), None)?;
        }
        dst.pragma_update(None, "journal_mode", "DELETE")?; // 备份文件自包含，无 WAL
        Ok(())
    })?;

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
                writeln!(
                    f,
                    "# Focusly 错误日志 — {} ({category})\n",
                    chrono::Local::now().format("%Y-%m-%d")
                )?;
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
        use crate::db::{migrations, Db};

        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::init(tmp.path().to_path_buf()).unwrap();
        let db = Db::open(&paths.db).unwrap();
        db.with(migrations::run).unwrap();

        // 建 7 份备份验证轮转（时间戳秒级分辨率，间隔 >1s 保证文件名唯一）
        for i in 0..7 {
            std::thread::sleep(std::time::Duration::from_millis(1100));
            db.with(|c| crate::db::notes::create(c, &format!("n{i}"), ""))
                .unwrap();
            backup_database(&db, &paths).unwrap();
        }
        let count = std::fs::read_dir(&paths.backups)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("database-"))
            .count();
        assert!(count <= 5, "备份应轮转，实际 {count} 份");

        // 最新备份内容可用（还原演练另有专项测试）
        let mut names: Vec<String> = std::fs::read_dir(&paths.backups)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
            .collect();
        names.sort();
        let latest = names.last().unwrap();
        assert!(std::fs::read(paths.backups.join(latest)).is_ok());
    }

    /// 备份恢复演练：写入 → 备份 → 破坏原库 → 用备份还原 → 数据完整。
    /// 这是"备份可恢复"的唯一权威验证，重构备份逻辑时不得删除。
    #[test]
    fn backup_restore_drill() {
        use crate::db::{migrations, Db};

        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::init(tmp.path().to_path_buf()).unwrap();
        let db = Db::open(&paths.db).unwrap();
        db.with(migrations::run).unwrap();
        db.with(|c| crate::db::notes::create(c, "演练便签", "重要数据"))
            .unwrap();

        backup_database(&db, &paths).unwrap();

        // 找最新备份
        let mut backups: Vec<PathBuf> = std::fs::read_dir(&paths.backups)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        backups.sort();
        let latest = backups.last().unwrap().clone();
        assert!(latest.is_file());

        // 破坏原库（模拟损坏）
        std::fs::write(&paths.db, b"corrupted junk data").unwrap();

        // 还原：关闭连接 → 拷贝备份覆盖 → 重开验证
        drop(db);
        std::fs::copy(&latest, &paths.db).unwrap();
        let db2 = Db::open(&paths.db).unwrap();
        let (title, content): (String, String) = db2
            .with(|c| {
                c.query_row("SELECT title, content FROM notes LIMIT 1", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .map_err(crate::error::AppError::from)
            })
            .unwrap();
        assert_eq!(title, "演练便签");
        assert_eq!(content, "重要数据");
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
    fn local_data_mode_detection() {
        let tmp = tempfile::tempdir().unwrap();
        // 无 data/ 目录且无 marker → 非本地模式
        assert!(!is_local_data_mode(tmp.path()));
        // 有 data/ 目录 → 本地模式
        std::fs::create_dir(tmp.path().join("data")).unwrap();
        assert!(is_local_data_mode(tmp.path()));
        // 有 portable.marker 也算
        let tmp2 = tempfile::tempdir().unwrap();
        std::fs::write(tmp2.path().join("portable.marker"), b"").unwrap();
        assert!(is_local_data_mode(tmp2.path()));
    }

    #[test]
    fn inside_root_check() {
        let root = Path::new("C:/data/Focusly/images");
        assert!(is_inside_root(
            root,
            Path::new("C:/data/Focusly/images/n1/a.png")
        ));
        assert!(!is_inside_root(
            root,
            Path::new("C:/Windows/system32/a.png")
        ));
    }
}
