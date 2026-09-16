//! 极简文件日志器：写入 logs/focusly.log。
//! 目的：长期后台运行时可排查问题，同时保持空载开销极低。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// 单文件超过 5MB 触发轮转
const ROTATE_BYTES: u64 = 5 * 1024 * 1024;

pub struct FileLogger {
    file: Mutex<Option<std::fs::File>>,
    path: PathBuf,
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} [{:5}] [{}] {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            record.level(),
            record.target(),
            record.args()
        );
        let mut guard = match self.file.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        // 文件被外部删除/轮转时自动重建
        if guard.is_none() {
            if let Ok(f) = OpenOptions::new().create(true).append(true).open(&self.path) {
                *guard = Some(f);
            }
        }
        if let Some(f) = guard.as_mut() {
            if f.write_all(line.as_bytes()).is_err() {
                *guard = None;
                return;
            }
            // 大小轮转：超过 5MB 归档为 focusly.log.1（只保留一代，避免占盘）
            if let Ok(meta) = f.metadata() {
                if meta.len() > ROTATE_BYTES {
                    *guard = None;
                    let rotated = self.path.with_extension("log.1");
                    let _ = std::fs::remove_file(&rotated);
                    let _ = std::fs::rename(&self.path, &rotated);
                    if let Ok(nf) = OpenOptions::new().create(true).append(true).open(&self.path) {
                        *guard = Some(nf);
                    }
                }
            }
        }
    }

    fn flush(&self) {}
}

/// 初始化全局日志。失败时静默降级（无日志但不影响功能）。
pub fn init(logs_dir: &PathBuf) {
    let path = logs_dir.join("focusly.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = OpenOptions::new().create(true).append(true).open(&path).ok();
    let logger = Box::new(FileLogger {
        file: Mutex::new(file),
        path,
    });
    let _ = log::set_boxed_logger(logger);
    log::set_max_level(log::LevelFilter::Info);
}
