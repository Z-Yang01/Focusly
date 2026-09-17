//! 中文自然语言时间解析（纯函数模块，无 Tauri 依赖，只依赖 chrono 与 RepeatType）。
//!
//! 入口：[`parse_time_nl`]。在含多余文字的输入中扫描提取第一个可解析时间段：
//! 先按空白/标点切 token，再对每个起点组合 1-4 个连续 token（长组合优先）尝试解析，
//! 返回最早（最左）成功组合。
//!
//! 语义决策（均有注释与测试佐证）：
//! - 相对时长（N分钟后/N小时后/N天后/半小时后）恒为未来，repeat=Once。
//! - 明天/后天/大后天/X月X日 按日期语义自然为未来（X月X日已过则 +1 年）。
//! - 周X/周末（可能落在今天）：若计算出的时刻 <= now 则 +7 天，保证严格未来。
//!   下周X = 下一个该周几（永不取今天）。
//! - 裸 "X点"（无时段）：0-23 按原值 24h 制，可能是过去时刻（调用方可用
//!   reminders::next_occurrence 前进），测试有注明。
//! - 下午/晚上 X点：X in 1..=11 加 12；晚上12点=次日00:00；下午12点=12:00；
//!   中午12点=12:00、中午1点=13:00；半夜12点=00:00 次日；上午/早上按原值。
//! - 日期部分缺省时间默认 09:00。
//! - 每天/每周X/每个工作日 分别映射 Daily/Weekly/Weekdays，其余 Once。
//!
//! 告警说明：入口 parse_time_nl 待提醒设置 UI 接线（见 lib.rs 命令表旁注释），
//! 接线前整模块暂未被生产路径调用，模块级压制 dead_code（纯函数已由文末单测覆盖）。
#![allow(dead_code)]

use chrono::{DateTime, Datelike, Duration, Local, LocalResult, NaiveDate, Utc, Weekday};

use crate::db::models::RepeatType;

/// 解析结果：触发时刻（UTC）与重复类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTime {
    pub at: DateTime<Utc>,
    pub repeat: RepeatType,
}

/// 解析中文自然语言时间。输入可含多余文字（如 "提醒我 明天下午3点 开会"）。
/// `now` 为当前时刻；解析在本地时区语义下进行（用户说"明天9点"指本地时间），
/// 返回值转成 UTC。无法解析返回 None。
pub fn parse_time_nl(input: &str, now: DateTime<Utc>) -> Option<ParsedTime> {
    let normalized = normalize(input);
    let tokens = tokenize(&normalized);
    if tokens.is_empty() {
        return None;
    }
    let n = tokens.len();
    // 最左起点优先；同一起点长组合优先（"明天 下午3点 开会" 优先吃满 "明天下午3点"）
    for start in 0..n {
        let max_len = 4.min(n - start);
        for len in (1..=max_len).rev() {
            let joined: String = tokens[start..start + len].concat();
            if let Some(p) = parse_compact(&joined, now) {
                return Some(p);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// 预处理：归一化 + 分词
// ---------------------------------------------------------------------------

/// 大小写/全半角空格/全角冒号/全角数字容错，并压平为小写。
fn normalize(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            '　' | '\u{00A0}' | '\t' | '\r' | '\n' => ' ',
            '：' => ':',
            '０'..='９' => {
                let v = c as u32 - 0xFEE0;
                char::from_u32(v).unwrap_or(c)
            }
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

/// token 切分符：空白 + 常用中英文标点。冒号保留（"9:30" 不切开）。
fn is_separator(c: char) -> bool {
    if c == ':' {
        return false;
    }
    if c.is_whitespace() {
        return true;
    }
    if c.is_ascii_punctuation() {
        return true;
    }
    matches!(
        c,
        '，' | '。'
            | '！'
            | '？'
            | '；'
            | '、'
            | '（'
            | '）'
            | '【'
            | '】'
            | '《'
            | '》'
            | '「'
            | '」'
            | '『'
            | '』'
            | '“'
            | '”'
            | '‘'
            | '’'
            | '…'
            | '—'
            | '～'
            | '·'
            | '〜'
    )
}

fn tokenize(s: &str) -> Vec<String> {
    s.split(is_separator)
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// 紧凑串（已去空白）整体解析
// ---------------------------------------------------------------------------

fn parse_compact(s: &str, now: DateTime<Utc>) -> Option<ParsedTime> {
    // 1) 相对时长：整串必须完全匹配
    if let Some(d) = parse_relative(s) {
        return Some(ParsedTime {
            at: now + d,
            repeat: RepeatType::Once,
        });
    }

    let today = now.with_timezone(&Local).date_naive();

    // 2) 重复前缀（"每周X" 自带周几日期语义）
    let (repeat, repeat_date, rest) = strip_repeat(s)?;

    // 3) 日期部分（可缺省；显式日期优先于重复自带的周几）
    let (date_hit, rest) = strip_date(rest);
    let date_hit = date_hit.or(repeat_date);

    // 4) 时刻部分（可缺省）
    let (tod, rest) = strip_time(rest);

    // 必须整串消费完毕，否则该组合不成立（扫描器会尝试其它组合）
    if !rest.is_empty() {
        return None;
    }

    // 5) 物化日期
    let mut date = resolve_date(date_hit.as_ref(), today)?;

    // Weekdays 重复且未显式给日期：周末顺延到周一
    if repeat == RepeatType::Weekdays && date_hit.is_none() {
        while date.weekday() == Weekday::Sat || date.weekday() == Weekday::Sun {
            date = date.succ_opt()?;
        }
    }

    // 晚上12点/半夜12点：无独立未来日期（即落在今天）时进位到次日 00:00
    if let Some(t) = tod {
        if t.midnight_shift && date == today {
            date = date.succ_opt()?;
        }
    }

    let (hour, minute) = tod.map_or((9u32, 0u32), |t| (t.hour, t.minute));
    let naive = date.and_hms_opt(hour, minute, 0)?;
    let local = match naive.and_local_timezone(Local) {
        LocalResult::Single(dt) => dt,
        LocalResult::Ambiguous(dt, _) => dt,
        LocalResult::None => return None,
    };
    let mut at = local.with_timezone(&Utc);

    // 周X/周末 可能落在今天（默认 09:00 已过）：+7 天保证严格未来
    if matches!(date_hit, Some(DateHit::PlainWeekday(_) | DateHit::Weekend)) {
        while at <= now {
            at += Duration::days(7);
        }
    }

    Some(ParsedTime { at, repeat })
}

// ---------------------------------------------------------------------------
// 相对时长
// ---------------------------------------------------------------------------

/// 解析 "N分钟后/N分钟後/N分后"、"N小时后/N个小时後"、"N天后"、"半小时后"。
/// 整串必须完全匹配（尾部不允许多余文字）。
fn parse_relative(s: &str) -> Option<Duration> {
    // 半小时系列：半小时后 / 半小時後 / 半个小时后 / 半個小時後
    for half in ["半个小时", "半個小時", "半小时", "半小時"] {
        if let Some(rest) = s.strip_prefix(half) {
            if matches_suffix_hou(rest) {
                return Some(Duration::minutes(30));
            }
        }
    }

    let (n, rest) = leading_number(s)?;
    if n > 99_999 {
        return None; // 防御：Duration 溢出 / 无意义大数
    }
    let (mult, rest) = if let Some(r) = strip_first(rest, &["分钟", "分鐘", "分"]) {
        (1i64, r)
    } else {
        // 可选 "个/個" + 小时/钟头；否则 天/日
        let no_ge = rest
            .strip_prefix('个')
            .or_else(|| rest.strip_prefix('個'))
            .unwrap_or(rest);
        match strip_first(no_ge, &["小时", "小時", "钟头", "鐘頭"]) {
            Some(r) => (60i64, r),
            None => {
                let r = strip_first(rest, &["天", "日"])?;
                (60 * 24, r)
            }
        }
    };
    if !matches_suffix_hou(rest) {
        return None;
    }
    Some(Duration::minutes(n as i64 * mult))
}

/// 剩余部分必须是 "后/後" 或 "之后/以後" 等并到串尾。
fn matches_suffix_hou(rest: &str) -> bool {
    let rest = rest
        .strip_prefix('之')
        .or_else(|| rest.strip_prefix('以'))
        .unwrap_or(rest);
    rest == "后" || rest == "後"
}

fn strip_first<'a>(s: &'a str, options: &[&str]) -> Option<&'a str> {
    options.iter().find_map(|o| s.strip_prefix(o))
}

/// 解析串首 ASCII 数字（全角数字已在 normalize 转换）。
fn leading_number(s: &str) -> Option<(u32, &str)> {
    let digits = s.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 || digits > 9 {
        return None;
    }
    let n: u32 = s[..digits].parse().ok()?;
    Some((n, &s[digits..]))
}

// ---------------------------------------------------------------------------
// 重复前缀
// ---------------------------------------------------------------------------

/// 剥离开头的重复关键词。"每周X" 必须后接周几，否则整体失败；
/// 周几同时作为日期语义返回（下一个该周几，物化后保证严格未来）。
fn strip_repeat(s: &str) -> Option<(RepeatType, Option<DateHit>, &str)> {
    for k in ["每天", "每日", "天天"] {
        if let Some(rest) = s.strip_prefix(k) {
            return Some((RepeatType::Daily, None, rest));
        }
    }
    for k in ["每星期", "每礼拜", "每周"] {
        if let Some(rest) = s.strip_prefix(k) {
            let (wd, used) = match_weekday(rest)?;
            return Some((
                RepeatType::Weekly,
                Some(DateHit::PlainWeekday(wd)),
                &rest[used..],
            ));
        }
    }
    for k in ["每个工作日", "每個工作日", "每个上班日", "工作日"] {
        if let Some(rest) = s.strip_prefix(k) {
            return Some((RepeatType::Weekdays, None, rest));
        }
    }
    Some((RepeatType::Once, None, s))
}

/// 匹配 [星期|礼拜|周]?X，返回 (周几, 消费字节数)。
fn match_weekday(s: &str) -> Option<(Weekday, usize)> {
    let mut rest = s;
    for p in ["星期", "礼拜", "周"] {
        if let Some(r) = rest.strip_prefix(p) {
            rest = r;
            break;
        }
    }
    let c = rest.chars().next()?;
    let wd = match c {
        '一' => Weekday::Mon,
        '二' => Weekday::Tue,
        '三' => Weekday::Wed,
        '四' => Weekday::Thu,
        '五' => Weekday::Fri,
        '六' => Weekday::Sat,
        '日' | '天' => Weekday::Sun,
        _ => return None,
    };
    Some((wd, s.len() - rest.len() + c.len_utf8()))
}

// ---------------------------------------------------------------------------
// 日期部分
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateHit {
    /// 相对今天偏移天数（含今天=0）
    Offset(i64),
    /// 周X（可落今天；物化后若 <= now 则 +7 天）
    PlainWeekday(Weekday),
    /// 下周X = 下一个该周几（永不取今天）
    NextWeekday(Weekday),
    /// 本月最后一天
    MonthEnd,
    /// 周末（今天为周末则取今天，否则下一个周六；物化后保证未来）
    Weekend,
    /// X月X日（年内已过则 +1 年）
    MonthDay(u32, u32),
}

fn strip_date(s: &str) -> (Option<DateHit>, &str) {
    let fixed = |hit: DateHit, used: usize| (Some(hit), &s[used..]);

    for (k, days) in [
        ("大后天", 3i64),
        ("大後天", 3),
        ("后天", 2),
        ("後天", 2),
        ("明天", 1),
        ("明日", 1),
        ("今天", 0),
        ("今日", 0),
    ] {
        if let Some(rest) = s.strip_prefix(k) {
            return fixed(DateHit::Offset(days), s.len() - rest.len());
        }
    }

    for p in ["下周", "下星期", "下礼拜"] {
        if let Some(rest) = s.strip_prefix(p) {
            if let Some((wd, used)) = match_weekday(rest) {
                return fixed(DateHit::NextWeekday(wd), s.len() - rest.len() + used);
            }
        }
    }

    if let Some((wd, used)) = match_weekday(s) {
        return fixed(DateHit::PlainWeekday(wd), used);
    }

    for k in ["月底", "月末"] {
        if let Some(rest) = s.strip_prefix(k) {
            return fixed(DateHit::MonthEnd, s.len() - rest.len());
        }
    }
    if let Some(rest) = s.strip_prefix("周末") {
        return fixed(DateHit::Weekend, s.len() - rest.len());
    }

    // X月X日 / X月X号（也支持 X月单数字日）
    if let Some((m, rest)) = leading_number(s) {
        if let Some(rest) = rest.strip_prefix('月') {
            if let Some((d, rest)) = leading_number(rest) {
                for suffix in ["日", "号"] {
                    if let Some(rest) = rest.strip_prefix(suffix) {
                        if (1..=12).contains(&m) && (1..=31).contains(&d) {
                            return fixed(DateHit::MonthDay(m, d), s.len() - rest.len());
                        }
                        return (None, s);
                    }
                }
            }
        }
    }

    (None, s)
}

/// 把日期语义物化为具体本地日期。
fn resolve_date(hit: Option<&DateHit>, today: NaiveDate) -> Option<NaiveDate> {
    match hit {
        None | Some(DateHit::Offset(0)) => Some(today),
        Some(DateHit::Offset(days)) => today.checked_add_signed(Duration::days(*days)),
        Some(DateHit::PlainWeekday(w)) => {
            let diff = weekday_diff(*w, today);
            today.checked_add_signed(Duration::days(diff))
        }
        Some(DateHit::NextWeekday(w)) => {
            let diff = weekday_diff(*w, today);
            let diff = if diff == 0 { 7 } else { diff };
            today.checked_add_signed(Duration::days(diff))
        }
        Some(DateHit::MonthEnd) => {
            let (y, m) = (today.year(), today.month());
            let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            NaiveDate::from_ymd_opt(ny, nm, 1)?.pred_opt()
        }
        Some(DateHit::Weekend) => {
            match today.weekday() {
                Weekday::Sat | Weekday::Sun => Some(today),
                _ => {
                    // 下一个周六
                    let sat_diff = (Weekday::Sat.num_days_from_monday() + 7
                        - today.weekday().num_days_from_monday())
                        % 7;
                    today.checked_add_signed(Duration::days(sat_diff as i64))
                }
            }
        }
        Some(DateHit::MonthDay(m, d)) => {
            let date = NaiveDate::from_ymd_opt(today.year(), *m, *d)?;
            if date < today {
                // 今年已过 → 明年
                NaiveDate::from_ymd_opt(today.year() + 1, *m, *d)
            } else {
                Some(date)
            }
        }
    }
}

/// 目标周几相对今天的天数差（0 = 今天）。
fn weekday_diff(w: Weekday, today: NaiveDate) -> i64 {
    ((w.num_days_from_monday() + 7 - today.weekday().num_days_from_monday()) % 7) as i64
}

// ---------------------------------------------------------------------------
// 时刻部分
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TimeOfDay {
    hour: u32,
    minute: u32,
    /// 晚上/半夜/凌晨 12 点 → 0 点且（若日期落在今天）进位次日
    midnight_shift: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DayPart {
    Am,       // 上午/早上：按原值
    Noon,     // 中午：12→12，1..=2→+12
    Pm,       // 下午：1..=11→+12，12→12
    Evening,  // 晚上：1..=11→+12，12→0(+shift)
    Midnight, // 半夜/凌晨：12→0(+shift)，其余原值
}

fn strip_time(s: &str) -> (Option<TimeOfDay>, &str) {
    // 形如 HH:MM（小时 1-2 位、分钟 2 位）
    if let Some((h, rest)) = leading_number(s) {
        if let Some(rest2) = rest.strip_prefix(':') {
            let digits = rest2.chars().take_while(|c| c.is_ascii_digit()).count();
            if digits == 2 {
                if let Ok(m) = rest2[..2].parse::<u32>() {
                    if h <= 23 && m <= 59 {
                        let consumed = s.len() - rest2.len() + 2;
                        return (
                            Some(TimeOfDay {
                                hour: h,
                                minute: m,
                                midnight_shift: false,
                            }),
                            &s[consumed..],
                        );
                    }
                }
            }
        }
    }

    // [时段] X点[半|整|M分|M]
    let (part, rest_after_part) = match_day_part(s);
    if let Some((n, rest)) = leading_number(rest_after_part) {
        if let Some(rest) = rest.strip_prefix('点') {
            if let Some((hour, minute, shift, rest)) = point_form(n, rest, part) {
                // rest 始终是 s 的后缀：已消费字节数 = s.len() - rest.len()
                let consumed = s.len() - rest.len();
                return (
                    Some(TimeOfDay {
                        hour,
                        minute,
                        midnight_shift: shift,
                    }),
                    &s[consumed..],
                );
            }
        }
    }
    (None, s)
}

/// 解析 "点" 之后的部分，返回 (时, 分, 是否午夜进位, 剩余串)。
fn point_form(n: u32, rest: &str, part: Option<DayPart>) -> Option<(u32, u32, bool, &str)> {
    if n > 23 {
        return None;
    }
    let (minute, rest) = if let Some(rest) = rest.strip_prefix('半') {
        (30u32, rest)
    } else if let Some(rest) = rest.strip_prefix('整') {
        (0, rest)
    } else if let Some((m, rest)) = leading_number(rest) {
        if m > 59 {
            return None;
        }
        (m, rest.strip_prefix('分').unwrap_or(rest))
    } else {
        (0, rest)
    };
    let (hour, shift) = apply_day_part(part, n);
    Some((hour, minute, shift, rest))
}

/// 剥离时段前缀。
fn match_day_part(s: &str) -> (Option<DayPart>, &str) {
    for (k, p) in [
        ("上午", DayPart::Am),
        ("早上", DayPart::Am),
        ("中午", DayPart::Noon),
        ("下午", DayPart::Pm),
        ("晚上", DayPart::Evening),
        ("半夜", DayPart::Midnight),
        ("凌晨", DayPart::Midnight),
    ] {
        if let Some(rest) = s.strip_prefix(k) {
            return (Some(p), rest);
        }
    }
    (None, s)
}

fn apply_day_part(part: Option<DayPart>, h: u32) -> (u32, bool) {
    match part {
        None | Some(DayPart::Am) => (h, false), // 裸 0-23 按原值；上午/早上按原值
        Some(DayPart::Noon) => {
            if (1..=2).contains(&h) {
                (h + 12, false)
            } else {
                (h, false)
            }
        }
        Some(DayPart::Pm) => {
            if (1..=11).contains(&h) {
                (h + 12, false)
            } else {
                (h, false) // 下午12点 = 12:00
            }
        }
        Some(DayPart::Evening) | Some(DayPart::Midnight) => {
            if h == 12 {
                (0, true) // 晚上/半夜/凌晨 12 点 = 00:00（次日语义）
            } else if (1..=11).contains(&h) && part == Some(DayPart::Evening) {
                (h + 12, false)
            } else {
                (h, false)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// 固定"现在"：本地 2026-09-15（周二）10:00。与机器时区无关。
    fn now_tue_10() -> DateTime<Utc> {
        Local
            .with_ymd_and_hms(2026, 9, 15, 10, 0, 0)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn local(y: i32, m: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Local
            .with_ymd_and_hms(y, m, d, h, mi, 0)
            .unwrap()
            .with_timezone(&Utc)
    }

    // ---- 相对时长 ----

    #[test]
    fn relative_minutes() {
        let p = parse_time_nl("30分钟后", now_tue_10()).unwrap();
        assert_eq!(p.at, local(2026, 9, 15, 10, 30));
        assert_eq!(p.repeat, RepeatType::Once);
    }

    #[test]
    fn relative_minutes_traditional_and_bare_fen() {
        assert_eq!(
            parse_time_nl("10分钟後", now_tue_10()).unwrap().at,
            local(2026, 9, 15, 10, 10)
        );
        assert_eq!(
            parse_time_nl("5分之后", now_tue_10()).unwrap().at,
            local(2026, 9, 15, 10, 5)
        );
    }

    #[test]
    fn relative_hours_and_days() {
        assert_eq!(
            parse_time_nl("3小时后", now_tue_10()).unwrap().at,
            local(2026, 9, 15, 13, 0)
        );
        assert_eq!(
            parse_time_nl("2个小时後", now_tue_10()).unwrap().at,
            local(2026, 9, 15, 12, 0)
        );
        assert_eq!(
            parse_time_nl("2天后", now_tue_10()).unwrap().at,
            local(2026, 9, 17, 10, 0)
        );
    }

    #[test]
    fn half_hour() {
        assert_eq!(
            parse_time_nl("半小时后", now_tue_10()).unwrap().at,
            local(2026, 9, 15, 10, 30)
        );
        assert_eq!(
            parse_time_nl("半个小时后", now_tue_10()).unwrap().at,
            local(2026, 9, 15, 10, 30)
        );
    }

    // ---- 时刻 ----

    #[test]
    fn colon_time_with_date() {
        let p = parse_time_nl("明天 9:30", now_tue_10()).unwrap();
        assert_eq!(p.at, local(2026, 9, 16, 9, 30));
        // 全角冒号
        assert_eq!(
            parse_time_nl("明天9：30", now_tue_10()).unwrap().at,
            local(2026, 9, 16, 9, 30)
        );
    }

    #[test]
    fn bare_hour_is_24h_value() {
        // 决策：无时段的裸 "X点" 按 24h 制原值；今天 9 点已过（now=10:00）仍返回原值
        let p = parse_time_nl("今天9点", now_tue_10()).unwrap();
        assert_eq!(p.at, local(2026, 9, 15, 9, 0));
        assert!(p.at < now_tue_10(), "裸时刻可能是过去，由调用方处理");
        // "15点" 原值
        assert_eq!(
            parse_time_nl("今天15点", now_tue_10()).unwrap().at,
            local(2026, 9, 15, 15, 0)
        );
    }

    #[test]
    fn dian_ban_and_fen() {
        assert_eq!(
            parse_time_nl("明天8点半", now_tue_10()).unwrap().at,
            local(2026, 9, 16, 8, 30)
        );
        assert_eq!(
            parse_time_nl("明天14点20分", now_tue_10()).unwrap().at,
            local(2026, 9, 16, 14, 20)
        );
        assert_eq!(
            parse_time_nl("明天下午3点整", now_tue_10()).unwrap().at,
            local(2026, 9, 16, 15, 0)
        );
    }

    #[test]
    fn afternoon_and_evening() {
        // 固定 now：明天下午3点
        let p = parse_time_nl("明天下午3点", now_tue_10()).unwrap();
        assert_eq!(p.at, local(2026, 9, 16, 15, 0));
        assert_eq!(
            parse_time_nl("晚上8点", now_tue_10()).unwrap().at,
            local(2026, 9, 15, 20, 0)
        );
        assert_eq!(
            parse_time_nl("下午3点半", now_tue_10()).unwrap().at,
            local(2026, 9, 15, 15, 30)
        );
    }

    #[test]
    fn noon_rules() {
        assert_eq!(
            parse_time_nl("中午12点", now_tue_10()).unwrap().at,
            local(2026, 9, 15, 12, 0)
        );
        assert_eq!(
            parse_time_nl("中午1点", now_tue_10()).unwrap().at,
            local(2026, 9, 15, 13, 0)
        );
    }

    #[test]
    fn midnight_12_is_next_day() {
        // 半夜12点 = 次日 00:00
        assert_eq!(
            parse_time_nl("半夜12点", now_tue_10()).unwrap().at,
            local(2026, 9, 16, 0, 0)
        );
        assert_eq!(
            parse_time_nl("晚上12点", now_tue_10()).unwrap().at,
            local(2026, 9, 16, 0, 0)
        );
    }

    // ---- 日期 ----

    #[test]
    fn day_words() {
        assert_eq!(
            parse_time_nl("明天", now_tue_10()).unwrap().at,
            local(2026, 9, 16, 9, 0)
        );
        assert_eq!(
            parse_time_nl("后天", now_tue_10()).unwrap().at,
            local(2026, 9, 17, 9, 0)
        );
        assert_eq!(
            parse_time_nl("大后天", now_tue_10()).unwrap().at,
            local(2026, 9, 18, 9, 0)
        );
        assert_eq!(
            parse_time_nl("今天", now_tue_10()).unwrap().at,
            local(2026, 9, 15, 9, 0)
        );
    }

    #[test]
    fn plain_weekday_always_future() {
        // 2026-09-15 是周二；"周三" → 下一个周三是明天 9/16 09:00（已是未来，不进位）
        assert_eq!(
            parse_time_nl("周三", now_tue_10()).unwrap().at,
            local(2026, 9, 16, 9, 0)
        );
        // "周二"（就是今天）：默认 09:00 已过 → +7 天到下周二
        assert_eq!(
            parse_time_nl("周二", now_tue_10()).unwrap().at,
            local(2026, 9, 22, 9, 0)
        );
        // "星期五" → 未来（本周五）
        assert_eq!(
            parse_time_nl("星期五", now_tue_10()).unwrap().at,
            local(2026, 9, 18, 9, 0)
        );
        // "礼拜天" → 周日
        assert_eq!(
            parse_time_nl("礼拜天", now_tue_10()).unwrap().at,
            local(2026, 9, 20, 9, 0)
        );
    }

    #[test]
    fn next_weekday_never_today() {
        // "下周一" = 下一个周一（2026-09-21）
        assert_eq!(
            parse_time_nl("下周一", now_tue_10()).unwrap().at,
            local(2026, 9, 21, 9, 0)
        );
        // "下周二"：今天就是周二，跳到下周
        assert_eq!(
            parse_time_nl("下周二", now_tue_10()).unwrap().at,
            local(2026, 9, 22, 9, 0)
        );
    }

    #[test]
    fn month_day_with_year_rollover() {
        assert_eq!(
            parse_time_nl("9月20日", now_tue_10()).unwrap().at,
            local(2026, 9, 20, 9, 0)
        );
        // 今年 1 月 1 日已过 → 明年
        let dec = Local
            .with_ymd_and_hms(2026, 12, 31, 12, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            parse_time_nl("1月1日", dec).unwrap().at,
            local(2027, 1, 1, 9, 0)
        );
    }

    #[test]
    fn month_end_and_weekend() {
        assert_eq!(
            parse_time_nl("月底", now_tue_10()).unwrap().at,
            local(2026, 9, 30, 9, 0)
        );
        // 2026-09-15 周二 → 下一个周六 09-19
        assert_eq!(
            parse_time_nl("周末", now_tue_10()).unwrap().at,
            local(2026, 9, 19, 9, 0)
        );
    }

    // ---- 重复 ----

    #[test]
    fn daily_with_time() {
        let p = parse_time_nl("每天 9:30", now_tue_10()).unwrap();
        assert_eq!(p.repeat, RepeatType::Daily);
        assert_eq!(p.at, local(2026, 9, 15, 9, 30));
    }

    #[test]
    fn weekly_with_weekday() {
        let p = parse_time_nl("每周三 8点", now_tue_10()).unwrap();
        assert_eq!(p.repeat, RepeatType::Weekly);
        assert_eq!(p.at, local(2026, 9, 16, 8, 0));
    }

    #[test]
    fn weekdays_repeat() {
        let p = parse_time_nl("每个工作日 9点", now_tue_10()).unwrap();
        assert_eq!(p.repeat, RepeatType::Weekdays);
        assert_eq!(p.at, local(2026, 9, 15, 9, 0));
        // 周六说 "工作日 9点" → 顺延到周一
        let sat = Local
            .with_ymd_and_hms(2026, 9, 19, 12, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        let p2 = parse_time_nl("工作日 9点", sat).unwrap();
        assert_eq!(p2.repeat, RepeatType::Weekdays);
        assert_eq!(p2.at, local(2026, 9, 21, 9, 0));
    }

    // ---- 扫描提取 ----

    #[test]
    fn extracts_time_from_prose() {
        let p = parse_time_nl("提醒我 明天下午3点 开会", now_tue_10()).unwrap();
        assert_eq!(p.at, local(2026, 9, 16, 15, 0));

        // 无空格 + 标点分隔
        let p2 = parse_time_nl("提醒我，明天下午3点，开会", now_tue_10()).unwrap();
        assert_eq!(p2.at, local(2026, 9, 16, 15, 0));

        // 组合 token："每天 9:30 站会" 吃满 "每天9:30"
        let p3 = parse_time_nl("每天 9:30 站会", now_tue_10()).unwrap();
        assert_eq!(p3.repeat, RepeatType::Daily);
        assert_eq!(p3.at, local(2026, 9, 15, 9, 30));
    }

    #[test]
    fn longest_combo_wins_at_same_start() {
        // "明天 下午3点 开会"：同一起点优先最长组合 → 明天 15:00，而非 "明天" 默认 09:00
        let p = parse_time_nl("明天 下午3点 开会", now_tue_10()).unwrap();
        assert_eq!(p.at, local(2026, 9, 16, 15, 0));
    }

    #[test]
    fn full_width_space_and_case() {
        let p = parse_time_nl("明天　下午3点", now_tue_10()).unwrap();
        assert_eq!(p.at, local(2026, 9, 16, 15, 0));
        let p2 = parse_time_nl("ABC 明天 9:30", now_tue_10()).unwrap();
        assert_eq!(p2.at, local(2026, 9, 16, 9, 30));
    }

    #[test]
    fn unparsable_returns_none() {
        assert!(parse_time_nl("", now_tue_10()).is_none());
        assert!(parse_time_nl("   ", now_tue_10()).is_none());
        assert!(parse_time_nl("开会 讨论事项 复盘", now_tue_10()).is_none());
        assert!(parse_time_nl("hello world 123", now_tue_10()).is_none());
        // "明天" 后跟无法解析的剩余 → 组合失败
        assert!(parse_time_nl("明天abc def ghi jkl", now_tue_10()).is_none());
    }

    // ---- 边界补缺（可靠性审查追加） ----

    #[test]
    fn boundary_midnight_0000() {
        // "0:00"：单数字小时 + 午夜零点合法（h=0 <= 23）→ 当天 00:00
        let p = parse_time_nl("0:00", now_tue_10()).unwrap();
        assert_eq!(p.at, local(2026, 9, 15, 0, 0));
    }

    #[test]
    fn boundary_last_minute_of_day_2359() {
        let p = parse_time_nl("23:59", now_tue_10()).unwrap();
        assert_eq!(p.at, local(2026, 9, 15, 23, 59));
    }

    #[test]
    fn boundary_noon_colon_1200() {
        // 冒号形式不做时段推断：12:00 就是正午 12 点
        let p = parse_time_nl("12:00", now_tue_10()).unwrap();
        assert_eq!(p.at, local(2026, 9, 15, 12, 0));
    }

    #[test]
    fn boundary_tomorrow_noon_is_noon_not_evening() {
        // "明天 12:00" = 正午 12 点；没有"下午"前缀不做 +12
        let p = parse_time_nl("明天 12:00", now_tue_10()).unwrap();
        assert_eq!(p.at, local(2026, 9, 16, 12, 0));
        assert_ne!(p.at, local(2026, 9, 17, 0, 0), "不是次日零点");
    }

    #[test]
    fn boundary_tonight_semantics() {
        // "今晚" 未收录为时段/日期关键词 → 整串无法消费 → None（当前语义 = 不解析）
        let now_23 = Local
            .with_ymd_and_hms(2026, 9, 15, 23, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert!(parse_time_nl("今晚", now_23).is_none(), "今晚 不是关键词");
        // 等价的已收录表达："晚上12点" 在 23:00 → 进位到次日 00:00（midnight_shift）
        let p = parse_time_nl("晚上12点", now_23).unwrap();
        assert_eq!(p.at, local(2026, 9, 16, 0, 0));
    }

    #[test]
    fn boundary_long_garbage_input() {
        // 超长输入（1000 字符无关键词垃圾）→ None，不 panic、不明显耗时
        let junk = "垃圾".repeat(1000);
        assert!(parse_time_nl(&junk, now_tue_10()).is_none());
    }

    #[test]
    fn boundary_full_width_digits_and_colon() {
        // 全角数字/冒号经 normalize 归一为半角后正常解析
        let p = parse_time_nl("１８：３０", now_tue_10()).unwrap();
        assert_eq!(p.at, local(2026, 9, 15, 18, 30));
    }
}
