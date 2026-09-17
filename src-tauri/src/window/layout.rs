//! 窗口布局：网格平铺计算、布局快照（反）序列化、布局应用与预设服务。
//! 坐标全部为物理像素；不动 scale 字段（几何按物理像素，跨 DPI 恢复由
//! set_position(Physical) 保证）。
//!
//! 未来命令接线（接线方在 commands/ 增加 layout_cmd.rs 并注册进 lib.rs）：
//! - layout_save_preset(name)      -> `save_preset(app, name)`（保存当前布局为预设）
//! - layout_list_presets()         -> `list_presets(app)`
//! - layout_apply_preset(id)       -> `apply_preset(app, id)`（返回应用槽位数）
//! - layout_delete_preset(id)      -> `delete_preset(app, id)`
//! - layout_arrange_grid(cols?)    -> `arrange_grid(app, cols)`（cols 为空按 sqrt 自适应）
//! 接线还需在 window/mod.rs 声明 `pub mod layout;`。

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

use crate::db::models::LayoutPreset;
use crate::error::AppResult;
use crate::state::AppState;

/// 网格默认间隙（物理像素）
const GRID_GAP: i32 = 12;

/// 单个便签的目标矩形（物理像素）。
#[derive(Debug, Clone)]
pub struct WindowSlot {
    pub note_id: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// 网格平铺：cols 列，行数 = ceil(n/cols)，最后一行左对齐，格子间留 gap。
/// - n=0 返回空 vec；
/// - cols=0 按 ceil(sqrt(n)) 自适应列数（至少 1）；
/// - cols 超过 n 时截断为 n（单行铺满）。
pub fn grid_slots(
    ids: &[String],
    area_x: i32,
    area_y: i32,
    area_w: i32,
    area_h: i32,
    gap: i32,
    cols: usize,
) -> Vec<WindowSlot> {
    let n = ids.len();
    if n == 0 {
        return Vec::new();
    }
    let effective_cols = if cols == 0 {
        (n as f64).sqrt().ceil() as usize
    } else {
        cols
    };
    let cols = effective_cols.clamp(1, n);
    let rows = n.div_ceil(cols);

    let usable_w = (area_w - gap * (cols as i32 - 1)).max(0);
    let usable_h = (area_h - gap * (rows as i32 - 1)).max(0);
    let cell_w = usable_w / cols as i32;
    let cell_h = usable_h / rows as i32;

    let mut slots = Vec::with_capacity(n);
    for (i, id) in ids.iter().enumerate() {
        let col = (i % cols) as i32;
        let row = (i / cols) as i32;
        slots.push(WindowSlot {
            note_id: id.clone(),
            x: area_x + col * (cell_w + gap),
            y: area_y + row * (cell_h + gap),
            w: cell_w,
            h: cell_h,
        });
    }
    slots
}

/// 序列化布局快照为紧凑 JSON：`[[noteId,x,y,w,h],...]`。
pub fn serialize_layout(entries: &[(String, i32, i32, i32, i32)]) -> String {
    serde_json::to_string(entries).unwrap_or_else(|_| "[]".into())
}

/// 反序列化布局快照；任何解析失败返回空 vec。
pub fn deserialize_layout(s: &str) -> Vec<(String, i32, i32, i32, i32)> {
    serde_json::from_str(s).unwrap_or_default()
}

/// 矩形中心落在哪个显示器的设备名（window/mod.rs 的 monitor_of_window 需要
/// 私有 HWND，这里按几何用 all_monitors 重新实现；找不到返回 None）。
#[cfg(windows)]
fn monitor_device_of(x: i32, y: i32, w: i32, h: i32) -> Option<String> {
    let (cx, cy) = (x + w / 2, y + h / 2);
    super::monitor::all_monitors()
        .into_iter()
        .find(|m| cx >= m.rect.left && cx < m.rect.right && cy >= m.rect.top && cy < m.rect.bottom)
        .map(|m| m.device)
}

#[cfg(not(windows))]
fn monitor_device_of(_x: i32, _y: i32, _w: i32, _h: i32) -> Option<String> {
    None
}

/// 主显示器矩形（物理像素）：保守用 Win32 枚举的第一个显示器矩形（不内缩任务栏）。
#[cfg(windows)]
fn primary_work_area(app: &AppHandle) -> (i32, i32, i32, i32) {
    if let Some(m) = super::monitor::all_monitors().first() {
        let r = m.rect;
        return (r.left, r.top, r.right - r.left, r.bottom - r.top);
    }
    tauri_primary_area(app)
}

#[cfg(not(windows))]
fn primary_work_area(app: &AppHandle) -> (i32, i32, i32, i32) {
    tauri_primary_area(app)
}

fn tauri_primary_area(app: &AppHandle) -> (i32, i32, i32, i32) {
    if let Ok(Some(m)) = app.primary_monitor() {
        let pos = m.position();
        let size = m.size();
        return (pos.x, pos.y, size.width as i32, size.height as i32);
    }
    (0, 0, 1280, 800)
}

/// 应用一批 slot：有打开窗口则 set_size/set_position（Physical，尺寸下限与
/// open_note_window 一致）并 update_geometry 写库；无窗口仅写库（下次
/// open_note_window 恢复位置）。不动 scale。
pub fn apply_layout_note(app: &AppHandle, slots: &[WindowSlot]) {
    let state = app.state::<AppState>();
    for slot in slots {
        let label = super::note_label(&slot.note_id);
        let monitor = monitor_device_of(slot.x, slot.y, slot.w, slot.h);
        if let Some(win) = app.get_webview_window(&label) {
            let _ = win.set_size(PhysicalSize::new(
                slot.w.max(120) as u32,
                slot.h.max(100) as u32,
            ));
            let _ = win.set_position(PhysicalPosition::new(slot.x, slot.y));
        }
        if let Err(e) = state.db.with(|c| {
            crate::db::notes::update_geometry(
                c,
                &slot.note_id,
                slot.x,
                slot.y,
                slot.w,
                slot.h,
                monitor.as_deref(),
            )
        }) {
            log::warn!("布局几何写库失败 {}: {e}", slot.note_id);
        }
    }
}

/// 抓取当前布局：遍历 active 未删便签，已开窗口用 outer_position/outer_size，
/// 无窗口（或读取失败）用 DB 几何；两者皆无则跳过 → serialize_layout。
pub fn capture_current_layout(app: &AppHandle) -> AppResult<String> {
    let state = app.state::<AppState>();
    let notes = state.db.with(|c| crate::db::notes::list(c, "active"))?;
    let mut entries: Vec<(String, i32, i32, i32, i32)> = Vec::with_capacity(notes.len());
    for summary in notes {
        let note = summary.note;
        let label = super::note_label(&note.id);
        let mut entry: Option<(String, i32, i32, i32, i32)> = None;
        if let Some(win) = app.get_webview_window(&label) {
            if let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) {
                entry = Some((
                    note.id.clone(),
                    pos.x,
                    pos.y,
                    size.width as i32,
                    size.height as i32,
                ));
            }
        }
        if entry.is_none() {
            if let (Some(x), Some(y), Some(w), Some(h)) = (note.x, note.y, note.width, note.height)
            {
                entry = Some((note.id.clone(), x, y, w, h));
            }
        }
        if let Some(e) = entry {
            entries.push(e);
        }
    }
    Ok(serialize_layout(&entries))
}

// ---------- 布局预设服务（layout_preset_svc 并入本文件） ----------

/// 保存当前布局为预设（同名覆盖）。
pub fn save_preset(app: &AppHandle, name: &str) -> AppResult<LayoutPreset> {
    let data = capture_current_layout(app)?;
    let state = app.state::<AppState>();
    let preset = state
        .db
        .with(|c| crate::db::layouts::save(c, name, &data))?;
    log::info!("布局预设已保存: {}", preset.name);
    Ok(preset)
}

/// 全部布局预设（按名称排序）。
pub fn list_presets(app: &AppHandle) -> AppResult<Vec<LayoutPreset>> {
    let state = app.state::<AppState>();
    state.db.with(crate::db::layouts::list)
}

/// 删除布局预设。
pub fn delete_preset(app: &AppHandle, id: &str) -> AppResult<()> {
    let state = app.state::<AppState>();
    state.db.with(|c| crate::db::layouts::delete(c, id))
}

/// 应用预设：反序列化 → apply_layout_note，返回应用槽位数（预设含已删便签时
/// 仅写库不弹窗，无害）。
pub fn apply_preset(app: &AppHandle, id: &str) -> AppResult<usize> {
    let state = app.state::<AppState>();
    let preset = state.db.with(|c| crate::db::layouts::get(c, id))?;
    let slots: Vec<WindowSlot> = deserialize_layout(&preset.data)
        .into_iter()
        .map(|(note_id, x, y, w, h)| WindowSlot {
            note_id,
            x,
            y,
            w,
            h,
        })
        .collect();
    let n = slots.len();
    apply_layout_note(app, &slots);
    log::info!("布局预设已应用: {}（{n} 个窗口）", preset.name);
    Ok(n)
}

/// 网格排列全部 active 未删便签到主显示器；cols=None/0 按 sqrt 自适应。
/// 返回排列的便签数。
pub fn arrange_grid(app: &AppHandle, cols: Option<usize>) -> AppResult<usize> {
    let ids: Vec<String> = {
        let state = app.state::<AppState>();
        let notes = state.db.with(|c| crate::db::notes::list(c, "active"))?;
        notes.into_iter().map(|s| s.note.id).collect()
    };
    let (area_x, area_y, area_w, area_h) = primary_work_area(app);
    let slots = grid_slots(
        &ids,
        area_x,
        area_y,
        area_w,
        area_h,
        GRID_GAP,
        cols.unwrap_or(0),
    );
    apply_layout_note(app, &slots);
    log::info!("网格排列完成: {} 个便签，cols={:?}", slots.len(), cols);
    Ok(slots.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("n{i}")).collect()
    }

    #[test]
    fn grid_empty_ids_returns_empty() {
        assert!(grid_slots(&[], 0, 0, 1000, 800, 8, 3).is_empty());
        assert!(grid_slots(&[], 0, 0, 1000, 800, 8, 0).is_empty());
    }

    #[test]
    fn grid_single_fills_whole_area() {
        let slots = grid_slots(&ids(1), 10, 20, 1000, 800, 8, 3);
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].note_id, "n0");
        assert_eq!(
            (slots[0].x, slots[0].y, slots[0].w, slots[0].h),
            (10, 20, 1000, 800)
        );
    }

    #[test]
    fn grid_two_columns_two_rows() {
        let slots = grid_slots(&ids(4), 0, 0, 1000, 800, 0, 2);
        assert_eq!(slots.len(), 4);
        assert_eq!(slots[0].x, 0);
        assert_eq!(slots[1].x, 500, "第 2 列");
        assert_eq!(slots[2].y, 400, "第 2 行");
        assert_eq!(slots[3].x, 500);
        assert_eq!(slots[3].y, 400);
    }

    #[test]
    fn grid_gap_reserved_between_cells() {
        let slots = grid_slots(&ids(2), 0, 0, 1000, 800, 10, 2);
        // usable_w = 1000 - 10 = 990，cell_w = 495；第 2 格 x = 495 + 10
        assert_eq!(slots[0].w, 495);
        assert_eq!(slots[1].x, 505);
        assert_eq!(slots[1].w, 495);
    }

    #[test]
    fn grid_uneven_last_row_left_aligned() {
        // 5 个、2 列 → 3 行，最后一行只有 1 个且左对齐，格宽不拉伸
        let slots = grid_slots(&ids(5), 0, 0, 1000, 900, 0, 2);
        assert_eq!(slots.len(), 5);
        let last = &slots[4];
        assert_eq!(last.x, 0, "最后一行左对齐");
        assert_eq!(last.y, 600, "第 3 行");
        assert_eq!(last.w, slots[0].w, "最后一行格宽不拉伸");
    }

    #[test]
    fn grid_cols_zero_uses_sqrt_adaptive() {
        // 9 个 → ceil(sqrt(9)) = 3 列
        let slots = grid_slots(&ids(9), 0, 0, 900, 900, 0, 0);
        assert_eq!(slots[1].x, 300);
        assert_eq!(slots[3].y, 300);
        // 5 个 → ceil(sqrt(5)) = 3 列 → 2 行，第 4 个在第二行第一列，行高 450
        let slots5 = grid_slots(&ids(5), 0, 0, 900, 900, 0, 0);
        assert_eq!(slots5[3].x, 0);
        assert_eq!(slots5[3].y, 450);
    }

    #[test]
    fn grid_cols_capped_at_count() {
        // 5 个、4 列 → 2 行（4 + 1），最后一行左对齐
        let slots = grid_slots(&ids(5), 0, 0, 800, 800, 0, 4);
        assert_eq!(slots[1].x, 200);
        assert_eq!(slots[4].x, 0);
        assert_eq!(slots[4].y, 400);
    }

    #[test]
    fn grid_area_offset_preserved() {
        let slots = grid_slots(&ids(2), 100, 50, 400, 200, 0, 2);
        assert_eq!((slots[0].x, slots[0].y), (100, 50));
        assert_eq!((slots[1].x, slots[1].y), (300, 50));
        assert_eq!((slots[1].w, slots[1].h), (200, 200));
    }

    #[test]
    fn serialize_deserialize_round_trip() {
        let entries = vec![
            ("a".to_string(), 1, 2, 3, 4),
            ("b".to_string(), -10, 20, 300, 400),
        ];
        let s = serialize_layout(&entries);
        assert_eq!(s, r#"[["a",1,2,3,4],["b",-10,20,300,400]]"#);
        assert_eq!(deserialize_layout(&s), entries);
    }

    #[test]
    fn serialize_deserialize_empty() {
        assert_eq!(serialize_layout(&[]), "[]");
        assert!(deserialize_layout("[]").is_empty());
    }

    #[test]
    fn deserialize_invalid_returns_empty() {
        assert!(deserialize_layout("not json").is_empty());
        assert!(deserialize_layout("{\"a\":1}").is_empty());
        assert!(deserialize_layout("[1,2,3]").is_empty());
        assert!(deserialize_layout("").is_empty());
    }
}
