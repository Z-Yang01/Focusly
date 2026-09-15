//! 极简文件日志器：写入 logs/focusly.log。
//! 目的：长期后台运行时可排查问题，同时保持空载开销极低。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

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
            let _ = f.write_all(line.as_bytes());
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
