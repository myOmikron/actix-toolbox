//! actix-toolbox enhances the functionality of actix-web by providing middlewares
//! or other components that are frequently used together with actix-web.
//!
//! This also includes an ORM. [rorm](https://github.com/myOmikron/rorm) is used for this
#![warn(missing_docs)]

/// Provides logging functionality e.g. sets up a configured logger
pub mod logging;
/// Provides a variety of different middlewares
pub mod tb_middleware;