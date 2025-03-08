#![feature(box_into_inner)]

mod async_context;
mod logger;

pub use crate::async_context::{
    context_as_string, execute_with_async_context, from_context, from_context_mut, AsyncContext,
};

pub use logger::ContextLogger;
