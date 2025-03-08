//! # Async Context Library
//!
//! This code was oringally written by Michel Smola and released under the MIT license.
//! The original crate can be found at https://docs.rs/async-context/0.1.1/async_context/index.html
//! It was adapted for use at Wonop by Troels Frimodt Rønnow.
//!
//! This library provides a mechanism for managing contextual data across async tasks
//! in Rust applications. It allows you to safely share state within the scope of a
//! single async execution context.
//!
//! ## Key Features
//!
//! - Thread-safe context management using thread-local storage
//! - Prevents nested context creation to avoid confusion and bugs
//! - Support for both immutable and mutable access to context data
//! - Type-safe context access through generics
//! - Automatically cleans up context when the async scope exits
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use with_async_context::{execute_with_async_context, from_context};
//!
//! #[derive(Clone)]
//! struct MyContext {
//!     some_value: String
//! }
//!
//! impl ToString for MyContext {
//!     fn to_string(&self) -> String {
//!         self.some_value.clone()
//!     }
//! }
//!
//! async fn my_function() {
//!     // Access the current context
//!     let value = from_context(|ctx: Option<&MyContext>| {
//!         ctx.unwrap().some_value.clone()
//!     });
//!
//!     // Do something with the value...
//! }
//!
//! # async fn example() {
//! let context = MyContext { some_value: "test".to_string() };
//! let result = execute_with_async_context(context, my_function()).await;
//! # }
//! ```
use core::future::Future;
use std::{any::Any, cell::RefCell, pin::Pin, rc::Rc, sync::Mutex, task::Poll};

use pin_project::pin_project;

thread_local! {
    static CONTEXT: RefCell<Option<Rc<RefCell<dyn Any>>>> = RefCell::new(None);
    static HAS_CONTEXT: RefCell<bool> = RefCell::new(false);
}

/// Represents an async execution context that carries data of type `C` while executing
/// a future of type `F` that produces output of type `T`
#[pin_project]
pub struct AsyncContext<C, T, F>
where
    C: 'static + ToString,
    F: Future<Output = T>,
{
    /// The context data wrapped in a mutex for thread-safety
    ctx: Mutex<Option<Rc<RefCell<C>>>>,

    /// The future being executed, marked with #[pin] for self-referential struct support
    #[pin]
    future: F,
}

/// Creates a new async context and executes the provided future within it
///
/// # Arguments
///
/// * `ctx` - The context data to make available during future execution
/// * `future` - The future to execute within the context
///
/// # Panics
///
/// Panics if attempting to create a nested context when one already exists
///
/// # Examples
///
/// ```rust,no_run
/// use with_async_context::execute_with_async_context;
///
/// struct MyContext;
///
/// impl MyContext {
///     fn new() -> Self {
///         MyContext
///     }
/// }
///
/// impl ToString for MyContext {
///     fn to_string(&self) -> String {
///         "MyContext".into()
///     }
/// }
///
/// async fn async_function() {}
///
/// # async fn example() {
/// let result = execute_with_async_context(
///     MyContext::new(),
///     async_function()
/// ).await;
/// # }
/// ```
pub fn execute_with_async_context<C, T, F>(ctx: C, future: F) -> AsyncContext<C, T, F>
where
    C: 'static + ToString,
    F: Future<Output = T>,
{
    // Prevent nested contexts
    if HAS_CONTEXT.with(|x| *x.borrow()) {
        panic!("Cannot create nested contexts.");
    }

    AsyncContext {
        ctx: Mutex::new(Some(Rc::new(RefCell::new(ctx)))),
        future,
    }
}

impl<C, T, F> Future for AsyncContext<C, T, F>
where
    C: 'static + ToString,
    F: Future<Output = T>,
{
    // Returns a tuple of the Future's output value and the context
    type Output = (T, Rc<RefCell<C>>);

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        // Take ownership of the context from the mutex, removing it temporarily
        let ctx: Rc<RefCell<C>> = self
            .ctx
            .lock()
            .expect("Failed to lock context mutex")
            .take()
            .expect("No context found");

        // Set thread-local flags to indicate context is now active
        HAS_CONTEXT.with(|x| *x.borrow_mut() = true);

        // Store context in thread local storage
        CONTEXT.with(|x| *x.borrow_mut() = Some(ctx.clone()));

        // Project the pinned future to get mutable access
        let projection = self.project();
        let future: Pin<&mut F> = projection.future;

        // Poll the inner future
        let poll = future.poll(cx);

        // Reset thread-local flag since we're done with this poll
        HAS_CONTEXT.with(|x| *x.borrow_mut() = false);
        CONTEXT.with(|x| *x.borrow_mut() = None);

        match poll {
            // If future is complete, return result with context
            Poll::Ready(value) => return Poll::Ready((value, ctx)),
            // If pending, restore context to mutex and return pending
            Poll::Pending => {
                projection
                    .ctx
                    .lock()
                    .expect("Failed to lock context mutex")
                    .replace(ctx);
                Poll::Pending
            }
        }
    }
}

/// Returns the current context as a string, or "(no context)" if none exists
pub fn context_as_string<C: 'static + ToString>() -> String {
    from_context(|ctx: Option<&C>| match ctx {
        Some(c) => c.to_string(),
        None => "(no context)".to_string(),
    })
}

/// Provides immutable access to the current context value
///
/// # Type Parameters
///
/// * `C` - The type of the context value to access
/// * `F` - The function type that will operate on the context
/// * `R` - The return type from the function
///
/// # Arguments
///
/// * `f` - A function that receives an Option<&C> and returns R
///
/// # Examples
///
/// ```rust,no_run
/// use with_async_context::from_context;
///
/// struct MyContext {
///     some_value: String
/// }
/// # fn example() {
/// let value = from_context(|ctx: Option<&MyContext>| {
///     ctx.unwrap().some_value.clone()
/// });
/// # }
/// ```
pub fn from_context<C, F, R>(f: F) -> R
where
    F: FnOnce(Option<&C>) -> R,
    C: 'static,
{
    CONTEXT.with(|value| match value.borrow().as_ref() {
        None => f(None),
        Some(ctx) => {
            let ctx_inner = ctx.borrow();
            let ctx_ref = ctx_inner
                .downcast_ref::<C>()
                .expect("Context type mismatch");
            f(Some(ctx_ref))
        }
    })
}

/// Provides mutable access to the current context value
///
/// # Type Parameters
///
/// * `C` - The type of the context value to access
/// * `F` - The function type that will operate on the context
/// * `R` - The return type from the function
///
/// # Arguments
///
/// * `f` - A function that receives an Option<&mut C> and returns R
///
/// # Examples
///
/// ```rust,no_run
/// use with_async_context::from_context_mut;
///
/// struct MyContext {
///     counter: i32
/// }
/// # fn example() {
/// from_context_mut(|ctx: Option<&mut MyContext>| {
///     if let Some(ctx) = ctx {
///         ctx.counter += 1;
///     }
/// });
/// # }
/// ```
pub fn from_context_mut<C, F, R>(f: F) -> R
where
    F: FnOnce(Option<&mut C>) -> R,
    C: 'static,
{
    CONTEXT.with(|value| {
        let mut binding = value.borrow_mut();
        let ctx = binding.as_mut().expect("No context found");
        let mut ctx_inner = ctx.borrow_mut();
        let ctx_ref = ctx_inner
            .downcast_mut::<C>()
            .expect("Context type mismatch");
        f(Some(ctx_ref))
    })
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, fmt::Display, sync::Arc, time::Duration};

    use tokio::time::sleep;

    use super::*;

    #[tokio::test]
    async fn test_basic_context() {
        async fn runs_with_context() -> String {
            let value = from_context(|value: Option<&String>| value.unwrap().clone());
            value
        }

        let async_context = execute_with_async_context("foobar".to_string(), runs_with_context());
        let (value, ctx) = async_context.await;

        assert_eq!("foobar", value);
        assert_eq!("foobar", &*ctx);
    }

    #[tokio::test]
    async fn test_mutable_context() {
        #[derive(Debug)]
        struct IntWrapper(RefCell<i32>);

        impl Display for IntWrapper {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0.borrow())
            }
        }

        async fn mutate_context() -> i32 {
            from_context(|value: Option<&IntWrapper>| {
                let val = value.unwrap();
                *val.0.borrow_mut() += 5;
                *val.0.borrow()
            })
        }

        let async_context =
            execute_with_async_context(IntWrapper(RefCell::new(10)), mutate_context());
        let (value, ctx) = async_context.await;

        assert_eq!(15, value);
        assert_eq!("15", ctx.to_string());
    }

    #[tokio::test]
    async fn test_complex_type() {
        #[derive(Debug, Clone, PartialEq)]
        struct TestStruct {
            name: String,
            count: i32,
        }

        impl Display for TestStruct {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}:{}", self.name, self.count)
            }
        }

        async fn use_complex_context() -> TestStruct {
            from_context(|value: Option<&TestStruct>| value.unwrap().clone())
        }

        let test_struct = TestStruct {
            name: "test".to_string(),
            count: 42,
        };

        let async_context = execute_with_async_context(test_struct.clone(), use_complex_context());
        let (value, ctx) = async_context.await;

        assert_eq!(test_struct, value);
        assert_eq!(test_struct, *ctx);
    }

    #[tokio::test]
    async fn test_arc_context() {
        #[derive(Debug)]
        struct ArcWrapper(Arc<i32>);

        impl Display for ArcWrapper {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", *self.0)
            }
        }

        async fn use_arc_context() -> i32 {
            from_context(|value: Option<&ArcWrapper>| *value.unwrap().0)
        }

        let arc_value = Arc::new(100);
        let async_context =
            execute_with_async_context(ArcWrapper(arc_value.clone()), use_arc_context());
        let (value, _) = async_context.await;

        assert_eq!(100, value);
    }

    #[tokio::test]
    #[should_panic(expected = "No context found")]
    async fn test_missing_context() {
        async fn runs_without_context() {
            from_context(|v: Option<&String>| v.cloned().expect("No context found"));
        }

        runs_without_context().await;
    }

    #[tokio::test]
    #[should_panic(expected = "Cannot create nested context")]
    async fn test_nested_contexts() {
        async fn inner_fn() -> String {
            let inner_val = from_context(|ctx: Option<&String>| ctx.unwrap().clone());
            sleep(Duration::from_millis(50)).await;
            inner_val
        }

        async fn outer_fn() -> String {
            let outer_val = from_context(|ctx: Option<&String>| ctx.unwrap().clone());
            let inner_context = execute_with_async_context("inner".to_string(), inner_fn()).await;
            format!("{}-{}", outer_val, inner_context.0)
        }

        let context = execute_with_async_context("outer".to_string(), outer_fn());
        let _ = context.await;
    }

    #[tokio::test]
    async fn test_context_persistence() {
        async fn task_with_delay() -> String {
            let val = from_context(|ctx: Option<&String>| ctx.unwrap().clone());
            sleep(Duration::from_millis(50)).await;
            let val2 = from_context(|ctx: Option<&String>| ctx.unwrap().clone());
            assert_eq!(val, val2);
            val
        }

        let context = execute_with_async_context("test".to_string(), task_with_delay());
        let (result, _) = context.await;
        assert_eq!("test", result);
    }

    #[tokio::test]
    async fn test_parallel_contexts() {
        #[derive(Debug)]
        struct IntWrapper(Arc<i32>);

        impl Display for IntWrapper {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", *self.0)
            }
        }

        async fn task(id: i32) -> i32 {
            let val = from_context(|ctx: Option<&IntWrapper>| *ctx.unwrap().0);
            sleep(Duration::from_millis(50)).await;
            val + id
        }

        let task1 = execute_with_async_context(IntWrapper(Arc::new(1)), task(10));
        let task2 = execute_with_async_context(IntWrapper(Arc::new(2)), task(20));
        let task3 = execute_with_async_context(IntWrapper(Arc::new(3)), task(30));

        let ((r1, _), (r2, _), (r3, _)) = tokio::join!(task1, task2, task3);

        assert_eq!(r1, 11);
        assert_eq!(r2, 22);
        assert_eq!(r3, 33);
    }

    #[tokio::test]
    async fn test_simple_nested_chains() {
        #[derive(Debug)]
        struct SimpleContext {
            value: i32,
        }

        impl Display for SimpleContext {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "Value: {}", self.value)
            }
        }

        fn nested_task(depth: i32) -> Pin<Box<dyn Future<Output = i32> + Send>> {
            Box::pin(async move {
                if depth == 0 {
                    return from_context(|ctx: Option<&SimpleContext>| ctx.unwrap().value);
                }

                sleep(Duration::from_millis(10)).await;
                nested_task(depth - 1).await + 1
            })
        }

        let context = SimpleContext { value: 42 };
        let (result, _) = execute_with_async_context(context, nested_task(3)).await;

        // Each level adds 1, so result should be 42 + 3
        assert_eq!(result, 45);
    }

    #[tokio::test]
    async fn test_value_chains() {
        #[derive(Debug)]
        struct NumberContext {
            value: Arc<i32>,
        }

        impl Display for NumberContext {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "Number: {}", *self.value)
            }
        }

        fn check_value(
            depth: i32,
            expected_value: i32,
        ) -> Pin<Box<dyn Future<Output = i32> + Send>> {
            Box::pin(async move {
                let ret = from_context(|ctx: Option<&NumberContext>| {
                    let value = *ctx.unwrap().value;
                    assert_eq!(value, expected_value, "Context value changed");
                    value
                });
                if depth == 0 {
                    return ret;
                }

                sleep(Duration::from_millis(1)).await;
                check_value(depth - 1, expected_value).await
            })
        }

        async fn run_value_chain(n: i32) -> i32 {
            let ctx = NumberContext { value: Arc::new(n) };
            // Run in local task to avoid Send requirement
            let result = tokio::task::LocalSet::new()
                .run_until(async move {
                    let (result, _) = execute_with_async_context(ctx, check_value(10, n)).await;
                    result
                })
                .await;
            result
        }

        let local = tokio::task::LocalSet::new();
        local.spawn_local(async {
            let mut chain_tasks = Vec::new();
            for i in 0..500 {
                let handle = tokio::task::spawn_local(run_value_chain(i));
                chain_tasks.push(handle);
            }

            let results = futures::future::join_all(chain_tasks).await;

            for (i, result) in results.into_iter().enumerate() {
                assert_eq!(result.unwrap(), i as i32);
            }
        });
        local.await;
    }

    #[tokio::test]
    #[should_panic(expected = "downcast failed")]
    async fn test_wrong_context_type() {
        #[derive(Debug)]
        struct Context1 {
            value: i32,
        }

        impl Display for Context1 {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.value)
            }
        }

        #[derive(Debug)]
        struct Context2 {
            text: String,
        }

        async fn access_wrong_type() {
            // Try to access Context2 when Context1 is active
            from_context(|ctx: Option<&Context2>| {
                let _ = ctx.unwrap();
            });
        }

        let ctx = Context1 { value: 42 };
        let context = execute_with_async_context(ctx, access_wrong_type());
        let _ = context.await;
    }
}
