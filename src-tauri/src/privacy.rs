//! 隐私防线纯函数：通知脱敏、摘要掩码、导出过滤。
//!
//! 约定：`is_private=1` 的便签（私密便签）正文不得出现在系统通知、
//! 搜索摘要与默认导出中。本模块只放与 Tauri/DB 无关的纯逻辑，
//! 便于单测；接线点（reminder.rs 通知、export.rs 查询）见各调用方。
//!
//! 注意：本模块需在 lib.rs 声明 `mod privacy;` 后才会参与编译
//! （声明与命令接线由总控完成）。

/// 私密便签通知的统一标题：原文标题不下发系统通知（标题也可能敏感）。
pub const PRIVATE_NOTICE_TITLE: &str = "Focusly 提醒";

/// 私密便签通知的固定占位 body。
pub const PRIVATE_NOTICE_BODY: &str = "🔒 私密便签内容已隐藏";

/// 私密便签在摘要位置（搜索命中/列表预览等）的固定掩码。
pub const PRIVATE_MASK: &str = "🔒 私密内容";

/// 通知文本脱敏：
/// - 私密便签：标题与正文都替换为固定占位（标题用应用名，不外泄原文标题）；
/// - 非私密：原样返回（title, content），由调用方自行截断摘要。
///
/// 调用点：reminder.rs `fire_due` 构造系统通知处（待总控接线）。
pub fn notification_text(title: &str, content: &str, is_private: bool) -> (String, String) {
    if is_private {
        (
            PRIVATE_NOTICE_TITLE.to_string(),
            PRIVATE_NOTICE_BODY.to_string(),
        )
    } else {
        (title.to_string(), content.to_string())
    }
}

/// 摘要掩码：私密便签的任何内容摘要（snippet）替换为固定占位；
/// 非私密原样返回。
pub fn mask_snippet(snippet: &str, is_private: bool) -> String {
    if is_private {
        PRIVATE_MASK.to_string()
    } else {
        snippet.to_string()
    }
}

/// 导出过滤：返回该便签是否应包含在导出中。
/// 默认（include_private=false）排除私密便签；用户显式选择包含时才放行。
pub fn export_filter_note(is_private: bool, include_private: bool) -> bool {
    include_private || !is_private
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_private_hides_body() {
        let (title, body) = notification_text("银行密码", "密码是 123456", true);
        assert_eq!(title, "Focusly 提醒");
        assert_eq!(body, "🔒 私密便签内容已隐藏");
    }

    #[test]
    fn notification_private_does_not_leak_title_or_content() {
        let (title, body) = notification_text("私密标题", "私密正文内容", true);
        assert!(!title.contains("私密标题"));
        assert!(!body.contains("私密正文内容"));
        assert!(!body.contains("私密标题"));
    }

    #[test]
    fn notification_public_passthrough() {
        let (title, body) = notification_text("购物清单", "# 内容\n- [ ] 买牛奶", false);
        assert_eq!(title, "购物清单");
        assert_eq!(body, "# 内容\n- [ ] 买牛奶");
    }

    #[test]
    fn notification_public_empty_strings_ok() {
        let (title, body) = notification_text("", "", false);
        assert_eq!(title, "");
        assert_eq!(body, "");
    }

    #[test]
    fn mask_snippet_private_replaces_with_placeholder() {
        assert_eq!(mask_snippet("任何摘要文本", true), "🔒 私密内容");
        assert_eq!(mask_snippet("", true), "🔒 私密内容");
    }

    #[test]
    fn mask_snippet_public_passthrough() {
        assert_eq!(
            mask_snippet("命中 <mark>关键词</mark> 摘要", false),
            "命中 <mark>关键词</mark> 摘要"
        );
        assert_eq!(mask_snippet("", false), "");
    }

    #[test]
    fn export_filter_default_excludes_private() {
        assert!(!export_filter_note(true, false), "默认导出不含私密便签");
    }

    #[test]
    fn export_filter_includes_private_when_opted_in() {
        assert!(export_filter_note(true, true), "显式选择包含时放行");
    }

    #[test]
    fn export_filter_keeps_public_regardless() {
        assert!(export_filter_note(false, false));
        assert!(export_filter_note(false, true));
    }
}
