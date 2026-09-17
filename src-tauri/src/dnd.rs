//! 通知勿扰时段（DND）与错过提醒汇总 — 纯逻辑模块，不触碰数据库与窗口，全部可单测。
//!
//! # 设置键（键名约定，settings.rs 的 DEFAULTS 不含这两键 = 默认关闭勿扰）
//! - `"dnd_start"`：勿扰开始时刻，`"HH:MM"`（本地时间，24 小时制）
//! - `"dnd_end"`：勿扰结束时刻，`"HH:MM"`
//!
//! 两键任一缺失 / 为空串 / 非法 → 视为未启用勿扰，始终通知（读取端容错，fail-open）。
//! `start == end` 视为未启用（零长度窗口无意义）。支持跨午夜窗口（如 22:00-07:00）。
//! 前端写入入口：`set_setting` 命令（启用写 "HH:MM"，关闭写 ""），见
//! `src/features/dnd/DndSettingsCard.tsx`。
//!
//! # 调度器接入点（reminder.rs 两处改动，由总控接线；此文件禁改 reminder.rs）
//!
//! 1. `fire_due`（reminder.rs ~L135，`for r in due` 循环内、过期兜底判断之后、
//!    读取便签/发通知之前）：会话开始时读一次设置，逐条判断：
//!
//!    ```ignore
//!    let settings = state
//!        .db
//!        .with(|c| crate::db::settings::get_all(c))
//!        .unwrap_or_default();
//!    let now_local = chrono::Local::now().naive_local();
//!    // 循环内，构造通知之前：
//!    if !crate::dnd::should_notify(&settings, now_local) {
//!        let w = crate::dnd::parse_window(
//!            settings.get("dnd_start").map(String::as_str).unwrap_or(""),
//!            settings.get("dnd_end").map(String::as_str).unwrap_or(""),
//!        );
//!        if let Some(w) = w {
//!            let exit_utc = crate::dnd::next_exit_utc(now, &w);
//!            if let Err(e) = state.db.with(|c| {
//!                crate::db::reminders::snooze_to(c, &r.id, &crate::reminder::fmt(exit_utc))
//!            }) {
//!                log::error!("勿扰期推迟提醒 {} 失败: {e}", r.id);
//!            }
//!        }
//!        continue; // 不发通知、不发 reminder-fired、不改 status（保持 pending）
//!    }
//!    ```
//!
//!    `snooze_to` 只把 remind_at 改到勿扰结束时刻并保持 `status='pending'`，
//!    状态机不变；loop 下一轮自动重算最近 pending 时间，无需 wake。
//!    注意 `next_exit_utc` 传 `now`（fire_due 顶部的 `Utc::now()`），保证与
//!    remind_at 同一时钟基准。
//!
//! 2. `spawn_loop`（reminder.rs ~L53，`tauri::async_runtime::spawn` 之前）：
//!    应用启动后发一条错过汇总系统通知：
//!
//!    ```ignore
//!    let missed = app.state::<AppState>().db.with(crate::db::missed::count_missed);
//!    if let Ok(Some(text)) = missed.map(|n| crate::dnd::missed_summary_text(n)) {
//!        let _ = app.notification().builder().title("Focusly").body(&text).show();
//!    }
//!    ```
//!
//!    读取失败时静默跳过（只记日志），不阻断调度器启动。

use std::collections::HashMap;

use chrono::{DateTime, Local, TimeZone, Utc};

/// settings 表中勿扰开始时刻的键（"HH:MM"；缺失/空 = 勿扰关闭，不进 DEFAULTS）
pub const DND_START_KEY: &str = "dnd_start";
/// settings 表中勿扰结束时刻的键（"HH:MM"；缺失/空 = 勿扰关闭，不进 DEFAULTS）
pub const DND_END_KEY: &str = "dnd_end";

/// 勿扰窗口：本地时间的起止时刻（分钟精度）。
/// `start == end` 视为未启用；`start > end` 表示跨午夜（如 22:00-07:00）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DndWindow {
    pub start_h: u8,
    pub start_m: u8,
    pub end_h: u8,
    pub end_m: u8,
}

/// 解析 "HH:MM" 为勿扰窗口。空串、非法格式、越界分钟/小时、start==end 均返回 None。
/// 前后空白容许（trim 后匹配）。
pub fn parse_window(start: &str, end: &str) -> Option<DndWindow> {
    let s = parse_hm(start)?;
    let e = parse_hm(end)?;
    if s == e {
        return None; // start==end 视为未启用
    }
    Some(DndWindow {
        start_h: s.0,
        start_m: s.1,
        end_h: e.0,
        end_m: e.1,
    })
}

/// "HH:MM" → (h, m)。严格两位（00-23 / 00-59），其余一律 None。
fn parse_hm(s: &str) -> Option<(u8, u8)> {
    let s = s.trim();
    let (h, m) = s.split_once(':')?;
    if h.len() != 2
        || m.len() != 2
        || !h.bytes().all(|b| b.is_ascii_digit())
        || !m.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let h: u8 = h.parse().ok()?;
    let m: u8 = m.parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some((h, m))
}

/// 当前本地时刻是否处于勿扰窗口内。
/// 左闭右开：start 时刻本身算勿扰内，end 时刻算勿扰外（勿扰结束时通知立即恢复）。
/// 未启用（None 或零长度窗口）→ false。
pub fn in_dnd(now_local: chrono::NaiveTime, w: &Option<DndWindow>) -> bool {
    let Some(w) = w else { return false };
    let Some(start) = chrono::NaiveTime::from_hms_opt(w.start_h as u32, w.start_m as u32, 0) else {
        return false;
    };
    let Some(end) = chrono::NaiveTime::from_hms_opt(w.end_h as u32, w.end_m as u32, 0) else {
        return false;
    };
    if start == end {
        return false; // 零长度窗口：未启用
    }
    if start < end {
        // 常规窗口（如 13:00-15:00）：start <= t < end
        now_local >= start && now_local < end
    } else {
        // 跨午夜窗口（如 22:00-07:00）：t >= start || t < end
        now_local >= start || now_local < end
    }
}

/// 勿扰结束的本地时刻（用于把到点提醒推迟到那时）。
/// NaiveTime 不带日期：跨午夜窗口的结束时刻在"明天"，日期偏移由
/// [`next_exit_utc`]（或调用方）处理。勿扰结束时通知恢复，即返回窗口 end。
pub fn next_exit_local(_now_local: chrono::NaiveTime, w: &DndWindow) -> chrono::NaiveTime {
    chrono::NaiveTime::from_hms_opt(w.end_h as u32, w.end_m as u32, 0)
        .unwrap_or(chrono::NaiveTime::MIN)
}

/// 勿扰结束对应的 UTC 时刻（调度器接线用：snooze_to 的目标时间）。
/// 结束时刻若已越过（<= now），视为"明天"的该时刻，保证结果严格大于 now。
/// 本地时区含 DST 缺口等换算失败时，退回 now + 1 小时（调度器下次仍会重推）。
pub fn next_exit_utc(now_utc: DateTime<Utc>, w: &DndWindow) -> DateTime<Utc> {
    let now_naive = now_utc.with_timezone(&Local).naive_local();
    let exit = next_exit_local(now_naive.time(), w);
    let mut target = now_naive.date().and_time(exit);
    if target <= now_naive {
        target += chrono::Duration::days(1);
    }
    Local
        .from_local_datetime(&target)
        .single()
        .map(|t| t.with_timezone(&Utc))
        .unwrap_or(now_utc + chrono::Duration::hours(1))
}

/// 是否应该发通知：读取 settings 的 "dnd_start"/"dnd_end"。
/// 未启用（缺失/空/非法/start==end）或当前不在窗口内 → true（始终通知，fail-open）。
pub fn should_notify(settings: &HashMap<String, String>, now_local: chrono::NaiveTime) -> bool {
    let window = parse_window(
        settings
            .get(DND_START_KEY)
            .map(String::as_str)
            .unwrap_or(""),
        settings.get(DND_END_KEY).map(String::as_str).unwrap_or(""),
    );
    !in_dnd(now_local, &window)
}

/// 错过汇总文案：count > 0 返回提示语，否则 None（不发通知）。
/// 计数来源见 `db::missed::count_missed`（近 7 天内自动取消且未触发的提醒）。
pub fn missed_summary_text(count: i64) -> Option<String> {
    if count > 0 {
        Some(format!(
            "📋 有 {count} 条错过/自动取消的提醒，打开管理器查看"
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn hm(s: &str) -> NaiveTime {
        NaiveTime::parse_from_str(s, "%H:%M").unwrap()
    }

    fn window(start: &str, end: &str) -> Option<DndWindow> {
        parse_window(start, end)
    }

    // ---------- parse_window ----------

    #[test]
    fn parse_window_accepts_valid() {
        let w = window("22:00", "07:00").expect("跨午夜窗口合法");
        assert_eq!(
            w,
            DndWindow {
                start_h: 22,
                start_m: 0,
                end_h: 7,
                end_m: 0
            }
        );
        let w2 = window("09:05", "18:45").unwrap();
        assert_eq!(w2.start_m, 5);
        assert_eq!(w2.end_m, 45);
    }

    #[test]
    fn parse_window_rejects_empty_and_garbage() {
        // 空串 / 缺失 → 未启用
        assert!(window("", "").is_none());
        assert!(window("", "07:00").is_none());
        assert!(window("22:00", "").is_none());
        // 非法格式
        assert!(window("22:0", "07:00").is_none(), "分钟必须两位");
        assert!(window("2:00", "07:00").is_none(), "小时必须两位");
        assert!(window("22-00", "07:00").is_none());
        assert!(window("ab:cd", "07:00").is_none());
        assert!(window("2200", "07:00").is_none());
        assert!(window("2200:30", "07:00").is_none());
        // 越界
        assert!(window("24:00", "07:00").is_none());
        assert!(window("22:60", "07:00").is_none());
        // 前后空白容许
        assert!(window(" 22:00 ", "07:00").is_some());
    }

    #[test]
    fn parse_window_rejects_equal_start_end() {
        assert!(window("13:00", "13:00").is_none(), "start==end 视为未启用");
        assert!(window("00:00", "00:00").is_none());
    }

    // ---------- in_dnd ----------

    #[test]
    fn in_dnd_disabled_is_false() {
        assert!(!in_dnd(hm("12:00"), &None));
        // start==end（直接构造绕过 parse_window）同样视为未启用
        let zero = Some(DndWindow {
            start_h: 5,
            start_m: 0,
            end_h: 5,
            end_m: 0,
        });
        assert!(!in_dnd(hm("05:00"), &zero));
    }

    #[test]
    fn in_dnd_normal_window_boundaries() {
        let w = window("13:00", "15:00");
        // start 时刻本身算勿扰内
        assert!(in_dnd(hm("13:00"), &w));
        assert!(in_dnd(hm("13:01"), &w));
        assert!(in_dnd(hm("14:59"), &w));
        // end 时刻算勿扰外（通知恢复）
        assert!(!in_dnd(hm("15:00"), &w));
        assert!(!in_dnd(hm("12:59"), &w));
        assert!(!in_dnd(hm("23:30"), &w));
    }

    #[test]
    fn in_dnd_cross_midnight() {
        let w = window("22:00", "07:00");
        assert!(!in_dnd(hm("21:59"), &w));
        assert!(in_dnd(hm("22:00"), &w), "start 时刻算勿扰内");
        assert!(in_dnd(hm("23:30"), &w));
        assert!(in_dnd(hm("00:00"), &w), "午夜之后仍属前一夜窗口");
        assert!(in_dnd(hm("03:15"), &w));
        assert!(in_dnd(hm("06:59"), &w));
        assert!(!in_dnd(hm("07:00"), &w), "end 时刻算勿扰外");
        assert!(!in_dnd(hm("12:00"), &w));
    }

    // ---------- next_exit_local / next_exit_utc ----------

    #[test]
    fn next_exit_local_returns_window_end() {
        let normal = window("13:00", "15:00").unwrap();
        assert_eq!(next_exit_local(hm("13:30"), &normal), hm("15:00"));
        let cross = window("22:00", "07:00").unwrap();
        assert_eq!(next_exit_local(hm("23:00"), &cross), hm("07:00"));
        assert_eq!(next_exit_local(hm("06:00"), &cross), hm("07:00"));
    }

    #[test]
    fn next_exit_utc_is_strictly_future_within_24h() {
        let now = Utc::now();
        for (s, e) in [("22:00", "07:00"), ("13:00", "15:00"), ("00:00", "23:59")] {
            let w = window(s, e).unwrap();
            let exit = next_exit_utc(now, &w);
            assert!(exit > now, "{s}-{e}: 推迟目标必须严格在未来");
            assert!(
                exit - now <= chrono::Duration::hours(24),
                "{s}-{e}: 不应推迟超过一天"
            );
        }
    }

    // ---------- should_notify ----------

    fn settings(start: &str, end: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert(DND_START_KEY.to_string(), start.to_string());
        m.insert(DND_END_KEY.to_string(), end.to_string());
        m
    }

    #[test]
    fn should_notify_when_not_configured() {
        let empty: HashMap<String, String> = HashMap::new();
        assert!(should_notify(&empty, hm("23:00")), "键不存在 = 始终通知");
        assert!(
            should_notify(&settings("", ""), hm("23:00")),
            "空串 = 始终通知"
        );
        assert!(should_notify(&settings("", "07:00"), hm("23:00")));
        assert!(should_notify(&settings("22:00", ""), hm("23:00")));
    }

    #[test]
    fn should_notify_fails_open_on_invalid_values() {
        assert!(should_notify(&settings("25:00", "07:00"), hm("23:00")));
        assert!(should_notify(&settings("22:00", "7:0"), hm("23:00")));
        assert!(should_notify(&settings("晚上", "早上"), hm("23:00")));
        assert!(
            should_notify(&settings("13:00", "13:00"), hm("13:00")),
            "start==end = 未启用"
        );
    }

    #[test]
    fn should_notify_suppressed_inside_window() {
        let cross = settings("22:00", "07:00");
        assert!(!should_notify(&cross, hm("22:00")), "start 时刻不通知");
        assert!(!should_notify(&cross, hm("02:30")));
        assert!(should_notify(&cross, hm("07:00")), "end 时刻恢复通知");
        assert!(should_notify(&cross, hm("12:00")));

        let normal = settings("13:00", "15:00");
        assert!(!should_notify(&normal, hm("14:00")));
        assert!(should_notify(&normal, hm("15:00")));
    }

    // ---------- missed_summary_text ----------

    #[test]
    fn missed_summary_text_positive_negative() {
        assert_eq!(
            missed_summary_text(3).as_deref(),
            Some("📋 有 3 条错过/自动取消的提醒，打开管理器查看")
        );
        assert_eq!(
            missed_summary_text(1).as_deref(),
            Some("📋 有 1 条错过/自动取消的提醒，打开管理器查看")
        );
        assert_eq!(missed_summary_text(0), None, "0 条不发通知");
        assert_eq!(missed_summary_text(-1), None);
    }

    // ---------- 边界补缺（可靠性审查追加） ----------

    #[test]
    fn boundary_cross_midnight_0659_is_inside() {
        // 22:00-07:00：06:59 仍在勿扰内（勿扰结束前 1 分钟）
        let w = window("22:00", "07:00");
        assert!(in_dnd(hm("06:59"), &w));
        assert!(!should_notify(&settings("22:00", "07:00"), hm("06:59")));
    }

    #[test]
    fn boundary_cross_midnight_0700_is_outside() {
        // 07:00 整点算勿扰外（左闭右开），通知立即恢复
        let w = window("22:00", "07:00");
        assert!(!in_dnd(hm("07:00"), &w));
        assert!(should_notify(&settings("22:00", "07:00"), hm("07:00")));
    }

    #[test]
    fn boundary_zero_window_0000_0000_disabled() {
        // 00:00-00:00：start==end 视为未启用，任何时刻都不判入勿扰
        assert!(parse_window("00:00", "00:00").is_none());
        let zero = Some(DndWindow {
            start_h: 0,
            start_m: 0,
            end_h: 0,
            end_m: 0,
        });
        assert!(!in_dnd(hm("00:00"), &zero));
        assert!(!in_dnd(hm("12:00"), &zero));
        assert!(should_notify(&settings("00:00", "00:00"), hm("00:00")));
    }

    #[test]
    fn boundary_cross_midnight_2300_0100() {
        // 窄跨午夜窗口 23:00-01:00，00:30 在窗口内
        let w = window("23:00", "01:00");
        assert!(in_dnd(hm("00:30"), &w));
        assert!(in_dnd(hm("23:00"), &w), "start 时刻算勿扰内");
        assert!(in_dnd(hm("00:00"), &w));
        assert!(!in_dnd(hm("01:00"), &w), "end 时刻算勿扰外");
        assert!(!in_dnd(hm("22:59"), &w));
    }

    #[test]
    fn should_notify_empty_settings_fails_open() {
        // 空设置（无任何键）：始终通知，fail-open
        let empty: HashMap<String, String> = HashMap::new();
        assert!(should_notify(&empty, hm("00:00")));
        assert!(should_notify(&empty, hm("12:00")));
        assert!(should_notify(&empty, hm("23:59")));
    }
}
