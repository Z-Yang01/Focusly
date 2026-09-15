//! 全局共享状态：由 lib.rs 在 setup 阶段 manage，各模块经 `app.state::<AppState>()` 访问。

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use crate::db::Db;
use crate::filesystem::AppPaths;

pub struct AppState {
    pub db: Db,
    pub paths: AppPaths,
    pub scheduler: crate::reminder::SchedulerHandle,
    /// 全局快捷键：规范化匹配键 -> 动作名（见 shortcut::register_all）
    pub shortcut_map: Mutex<HashMap<String, String>>,
    /// 被全屏策略隐藏的窗口 label（退出全屏后需要恢复显示）
    pub fullscreen_hidden: Mutex<HashSet<String>>,
    /// 几何去抖代数：label -> 最新代数，仅最新代的定时器允许写库
    pub geometry_gens: Mutex<HashMap<String, u64>>,
}
