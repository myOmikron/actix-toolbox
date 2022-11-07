use actix_web::middleware::Logger;

use crate::logging::LOG_PATTERN_ACTIX_NGINX_LIKE;

/**
Configuration for the Logger middleware.

Provides a default via the [Default] trait.
*/
#[derive(Clone, Debug)]
pub struct LoggingMiddlewareConfig {
    /// Pattern to use in logger. Defaults to [LOG_PATTERN_ACTIX_NGINX_LIKE]
    pub pattern: String,
    /// Logging target. Defaults to "requests"
    pub logging_target: String,
}

impl Default for LoggingMiddlewareConfig {
    fn default() -> Self {
        LoggingMiddlewareConfig {
            pattern: LOG_PATTERN_ACTIX_NGINX_LIKE.to_string(),
            logging_target: "requests".to_string(),
        }
    }
}

/**
Sets up a logging middleware with the given config.
*/
pub fn setup_logging_mw(config: LoggingMiddlewareConfig) -> Logger {
    Logger::new(&config.pattern).log_target(config.logging_target)
}
