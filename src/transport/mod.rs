use std::sync::Arc;
use async_trait::async_trait;
use anyhow::Result;
use crate::types::{JsonRpcRequest, JsonRpcResponse, TaskStreamingEvent};

pub mod nats;

// Create a trait for the handler function without generics
#[async_trait]
pub trait RequestHandler: Send + Sync {
    async fn handle(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse>;
}

// Implement RequestHandler for closures
struct FunctionHandler<F> {
    f: F,
}

#[async_trait]
impl<F, Fut> RequestHandler for FunctionHandler<F>
where
    F: Fn(JsonRpcRequest) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<JsonRpcResponse>> + Send + 'static,
{
    async fn handle(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        (self.f)(request).await
    }
}

// Make Transport trait object-safe by removing generics
#[async_trait]
pub trait Transport: Send + Sync {
    /// Start the transport, listening for incoming requests
    async fn start(&self) -> Result<()>;
    
    /// Stop the transport, terminating all connections
    async fn stop(&self) -> Result<()>;
    
    /// Run the transport with the provided request handler
    async fn run(&self, handler: Arc<dyn RequestHandler>) -> Result<()>;
    
    /// Publish a streaming event to the specified subscriber
    async fn publish_event(&self, reply_subject: &str, event: TaskStreamingEvent) -> Result<()>;
}

/// Factory trait for creating transport instances
pub trait TransportFactory: Send + Sync {
    /// Create a new transport instance
    fn create(&self) -> Result<Arc<dyn Transport>>;
}

// Helper function to create a RequestHandler from a closure
pub fn create_handler<F, Fut>(f: F) -> Arc<dyn RequestHandler>
where
    F: Fn(JsonRpcRequest) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<JsonRpcResponse>> + Send + 'static,
{
    Arc::new(FunctionHandler { f })
}
