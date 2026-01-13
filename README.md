# With Async Context

A Rust library providing thread-safe, type-safe context management across asynchronous tasks. It allows passing contextual data through middleware and async functions while maintaining proper lifetimes and preventing common pitfalls.

## Problem Solved

This library provides a way to pass context data through async Rust code without having to manually thread it through every function call. In server applications, you often need to track data like:

- Request IDs
- User authentication details
- IP addresses
- Request timing metrics
- Feature flags
- Account or tenant IDs

Passing this data as function parameters can make code messy and hard to maintain as applications grow. This library provides a simpler approach - it lets you wrap async operations in a context that makes the data available wherever needed in that request or task.

The context is accessed through helper functions, avoiding the need to pass parameters through multiple layers of code. It works well with logging to help track what's happening in your application.

You can use it for web APIs, background jobs, or any async Rust code where you need to maintain contextual data while keeping the benefits of Rust's type system and async support.

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
with-async-context = "0.1.2"
```

## Core Concepts

The library is built around a few key functions:

- `with_async_context`: Establishes a new context scope and runs a future within it
- `from_context`: Retrieves an immutable reference to the current context
- `from_context_mut`: Gets a mutable reference to modify the context
- `context_as_string`: Converts the context to a string representation

Here's a basic example showing the main patterns:

```rust
use with_async_context::{with_async_context, from_context};

#[derive(Clone)]
struct MyContext {
    value: String
}

impl ToString for MyContext {
    fn to_string(&self) -> String {
        self.value.clone()
    }
}

async fn my_function() -> String {
    from_context(|ctx: Option<&MyContext>| {
        ctx.unwrap().value.clone()
    })
}

async fn main() {
    let context = MyContext {
        value: "test".to_string()
    };

    let (result, _ctx) = with_async_context(context, my_function()).await;
    assert_eq!(result, "test");
}
```

## Web Framework Integration

The library seamlessly integrates with web frameworks like Axum for request context handling:

```rust
use with_async_context::{with_async_context, from_context};

#[derive(Clone)]
pub struct RequestContext {
    pub request_id: String,
    pub method: String,
    pub path: String,
}

impl ToString for RequestContext {
    fn to_string(&self) -> String {
        format!(
            "request_id={},method={},path={}",
            self.request_id, self.method, self.path
        )
    }
}

async fn axum_request_context(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();

    let context = RequestContext {
        request_id: Uuid::new_v4().to_string(),
        method: method.to_string(),
        path: path.to_owned(),
    };

    let response = with_async_context(context, async move {
        let response = next.run(req).await;
        response
    })
    .await
    .0;

    response
}

// Add the middleware to your Axum router
app.layer(axum::middleware::from_fn(axum_request_context))
```

Access the context from any async handler:

```rust
async fn handler() {
    let ctx = from_context(|ctx: Option<&RequestContext>| {
        ctx.unwrap().request_id.clone()
    });
    // Use context data...
}
```

## Structured Logging

The library includes a logging implementation that automatically incorporates context:

```rust
use with_async_context::{ContextLogger, init_context_logger};

// Initialize with default format
init_context_logger!(RequestContext);

// Or customize the format
init_context_logger!(RequestContext, "[{level}] {context} | {message}");

// Logs automatically include context details
log::info!("Processing request");
// Outputs: "INFO - request_id=123,method=GET,path=/users - Processing request"
```

## Safe Error Handling

The library encourages robust error handling patterns:

```rust
// Safely handle missing context with defaults
let value = from_context(|ctx: Option<&MyContext>| {
    ctx.map(|c| c.value.clone())
       .unwrap_or_default()
});

// Use Result for explicit error handling
let result = from_context(|ctx: Option<&MyContext>| {
    match ctx {
        Some(c) => Ok(c.value.clone()),
        None => Err("Context not found in current scope")
    }
});
```

## Thread Safety and Async

All operations are thread-safe and can be used reliably across:
- Async tasks and futures
- Thread pools
- Web request handlers
- Background jobs

The library prevents nested context creation to maintain clear ownership semantics and avoid common pitfalls in async code.

## License

Licensed under MIT.
