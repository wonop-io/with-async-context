//! # Context Logger Implementation
//!
//! This module provides a logging implementation that automatically includes the current
//! async context in log messages. It integrates with the standard `log` crate and allows
//! customizing the log format and level.
//!
//! ## Features
//!
//! - Thread-safe logging with context information
//! - Configurable log format with placeholders for level, context, and message
//! - Log level control via RUST_LOG environment variable
//! - Implementation of the standard `log::Log` trait
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use with_async_context::{ContextLogger, init_context_logger};
//!
//! #[derive(Clone)]
//! struct MyContext {
//!     request_id: String
//! }
//!
//! impl ToString for MyContext {
//!     fn to_string(&self) -> String {
//!         self.request_id.clone()
//!     }
//! }
//!
//! // Initialize the logger with default format
//! init_context_logger!(MyContext);
//!
//! // Or with custom format
//! init_context_logger!(MyContext, "[{level}] Request {context}: {message}");
//! ```

use std::{env, io::Write};

use crate::context_as_string;
use log::{Level, Log, Metadata, Record};

/// A logger implementation that includes async context information in log messages
pub struct ContextLogger<C>
where
    C: 'static + ToString + std::marker::Send + std::marker::Sync,
{
    /// Phantom data to hold the context type parameter
    pub _phantom: std::marker::PhantomData<C>,
    /// Optional custom format string for log messages
    format: Option<String>,
    /// Current log level threshold
    level: Option<Level>,
}

impl<C> ContextLogger<C>
where
    C: 'static + ToString + std::marker::Send + std::marker::Sync,
{
    /// Creates a new uninitialized context logger
    pub const fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
            format: None,
            level: None,
        }
    }

    /// Initializes the logger with an optional custom format
    ///
    /// # Arguments
    ///
    /// * `format` - Optional custom format string with {level}, {context}, and {message} placeholders
    pub fn init(&mut self, format: Option<String>) {
        let level = match env::var("RUST_LOG")
            .unwrap_or_default()
            .to_uppercase()
            .as_str()
        {
            "ERROR" => Level::Error,
            "WARN" => Level::Warn,
            "INFO" => Level::Info,
            "DEBUG" => Level::Debug,
            "TRACE" => Level::Trace,
            _ => Level::Info,
        };

        self.format =
            Some(format.unwrap_or_else(|| String::from("{level} - {context} - {message}")));
        self.level = Some(level);
    }
}

impl<C> Log for ContextLogger<C>
where
    C: 'static + ToString + std::marker::Send + std::marker::Sync,
{
    /// Checks if a log level is enabled
    fn enabled(&self, metadata: &Metadata) -> bool {
        if let Some(level) = self.level {
            metadata.level() <= level
        } else {
            false
        }
    }

    /// Logs a message with the current context
    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let context = context_as_string::<C>();

            if let Some(format) = &self.format {
                let msg = format
                    .replace("{level}", &record.level().to_string())
                    .replace("{context}", &context)
                    .replace("{message}", &record.args().to_string());

                let _ = std::io::stderr().write_all(msg.as_bytes());
                let _ = std::io::stderr().write_all(b"\n");
            }
        }
    }

    /// Flushes any buffered records
    fn flush(&self) {}
}

/// Macro to initialize the context logger
///
/// # Arguments
///
/// * `$context_type` - The type implementing the context
/// * `$format` - Optional custom format string
///
/// # Examples
///
/// ```rust,no_run
/// use with_async_context::{ContextLogger, init_context_logger};
///
/// #[derive(Debug)]
/// struct MyContext;
/// impl ToString for MyContext {
///    fn to_string(&self) -> String {
///       format!("{:?}", self)
///   }
/// }
///
/// // Initialize with default format
/// init_context_logger!(MyContext);
///
/// // Initialize with custom format
/// init_context_logger!(MyContext, "{level} [{context}] {message}");
/// ```
#[macro_export]
macro_rules! init_context_logger {
    ($context_type:ty) => {{
        use with_async_context::ContextLogger;
        static mut LOGGER: ContextLogger<$context_type> = ContextLogger::<$context_type>::new();
        unsafe {
            LOGGER.init(None);
            let max_level = match std::env::var("RUST_LOG")
                .unwrap_or_default()
                .to_uppercase()
                .as_str()
            {
                "ERROR" => log::LevelFilter::Error,
                "WARN" => log::LevelFilter::Warn,
                "INFO" => log::LevelFilter::Info,
                "DEBUG" => log::LevelFilter::Debug,
                "TRACE" => log::LevelFilter::Trace,
                _ => log::LevelFilter::Info,
            };
            let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(max_level));
        }
    }};
    ($context_type:ty, $format:expr) => {{
        use with_async_context::ContextLogger;
        static mut LOGGER: ContextLogger<$context_type> = ContextLogger::<$context_type>::new();
        unsafe {
            LOGGER.init(Some($format.to_string()));
            let max_level = match std::env::var("RUST_LOG")
                .unwrap_or_default()
                .to_uppercase()
                .as_str()
            {
                "ERROR" => log::LevelFilter::Error,
                "WARN" => log::LevelFilter::Warn,
                "INFO" => log::LevelFilter::Info,
                "DEBUG" => log::LevelFilter::Debug,
                "TRACE" => log::LevelFilter::Trace,
                _ => log::LevelFilter::Info,
            };
            let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(max_level));
        }
    }};
}
