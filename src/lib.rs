//! actix-toolbox enhances the functionality of actix-web by providing middlewares
//! or other components that are frequently used together with actix-web.
//!
//! This also includes an ORM. [rorm](https://github.com/rorm-orm/rorm) is used for this
#![warn(missing_docs)]

/// Provides logging functionality e.g. sets up a configured logger
#[cfg(feature = "logging")]
pub mod logging;
/// Provides a variety of different middlewares
pub mod tb_middleware;

/// Provides a sender-receiver based websocket interface
#[cfg(feature = "ws")]
pub mod ws;

/// Provides two handlers for the Open ID Connect protocol
#[cfg(feature = "oidc")]
pub mod oidc;
