# With Async Context

A Rust library for managing contextual data across async tasks. Enables easily passing request context through middleware and async functions.

## Features

- Thread-safe context management using thread-local storage
- Prevention of nested contexts to avoid confusion and bugs
- Both immutable and mutable access to context data
- Type-safe context access through generics
- Automatic cleanup when async scope exits
- Integrates with logging frameworks
- Context propagation across async boundaries

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
with-async-context = "0.1"
```

## Basic Usage

The library provides several key functions for working with async contexts:

- `execute_with_async_context`: Creates a new context and executes a future within it
- `from_context`: Access the current context immutably
- `from_context_mut`: Access the current context mutably
- `context_as_string`: Get the current context as a string representation

Here's a simple example:

```rust
use with_async_context::{execute_with_async_context, from_context};

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

    let (result, _ctx) = execute_with_async_context(context, my_function()).await;
    assert_eq!(result, "test");
}
```

## Example with Axum

The library integrates well with web frameworks like Axum for request context management:

```rust
use with_async_context::{execute_with_async_context, from_context};

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

    let response = execute_with_async_context(context, async move {
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

The context can then be accessed from any async function within the request:

```rust
async fn handler() {
    let ctx = from_context(|ctx: Option<&RequestContext>| {
        ctx.unwrap().request_id.clone()
    });
    // Use context data...
}
```

## Logging Integration

The library includes a logging implementation that automatically includes context information:

```rust
use with_async_context::{ContextLogger, init_context_logger};

// Initialize with default format
init_context_logger!(RequestContext);

// Or with custom format
init_context_logger!(RequestContext, "[{level}] Request {context}: {message}");

// Logs will include context info
log::info!("Processing request"); // Outputs: "INFO - request_id=123,method=GET,path=/users - Processing request"
```

## Error Handling

The library provides safe error handling through Options and Results:

```rust
// Safe unwrapping with default
let value = from_context(|ctx: Option<&MyContext>| {
    ctx.map(|c| c.value.clone()).unwrap_or_default()
});

// Pattern matching
let result = from_context(|ctx: Option<&MyContext>| {
    match ctx {
        Some(c) => Ok(c.value.clone()),
        None => Err("No context found")
    }
});
```

## Thread Safety

All context operations are thread-safe and can be used across async tasks and thread pools. The library prevents nested contexts to avoid confusion and potential bugs.

## License

MIT Licensed.
