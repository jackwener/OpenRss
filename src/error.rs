use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Route not found: {0}")]
    RouteNotFound(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Feed render error: {0}")]
    Render(String),

    #[error("{0}")]
    Internal(String),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Render(e.to_string())
    }
}

impl From<quick_xml::Error> for AppError {
    fn from(e: quick_xml::Error) -> Self {
        AppError::Render(e.to_string())
    }
}

impl From<quick_xml::DeError> for AppError {
    fn from(e: quick_xml::DeError) -> Self {
        AppError::Render(e.to_string())
    }
}

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;

        let status = match &self {
            AppError::RouteNotFound(_) => StatusCode::NOT_FOUND,
            AppError::Config(_) => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, self.to_string()).into_response()
    }
}
