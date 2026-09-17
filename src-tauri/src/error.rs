use serde::ser::Serializer;
use serde::Serialize;

/// 应用统一错误类型：前端可理解、可序列化、可记录日志。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(String),
    #[error("文件系统错误: {0}")]
    Io(String),
    #[error("窗口操作失败: {0}")]
    Window(String),
    #[error("图片处理失败: {0}")]
    Image(String),
    #[error("无效输入: {0}")]
    Invalid(String),
    #[error("资源不存在: {0}")]
    NotFound(String),
    #[error("快捷键错误: {0}")]
    Shortcut(String),
    #[error("系统能力不可用: {0}")]
    Platform(String),
    #[error("Tauri 错误: {0}")]
    Tauri(String),
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::Db(_) => "database",
            AppError::Io(_) => "io",
            AppError::Window(_) => "window",
            AppError::Image(_) => "image",
            AppError::Invalid(_) => "invalid",
            AppError::NotFound(_) => "not_found",
            AppError::Shortcut(_) => "shortcut",
            AppError::Platform(_) => "platform",
            AppError::Tauri(_) => "tauri",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        AppError::Tauri(e.to_string())
    }
}

impl From<image::ImageError> for AppError {
    fn from(e: image::ImageError) -> Self {
        AppError::Image(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
