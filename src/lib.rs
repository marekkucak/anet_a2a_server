// src/lib.rs

// Define modules
pub mod redis_task_manager;
pub mod task_manager;
pub mod task_manager_trait;
pub mod server;
pub mod server_builder;
pub mod transport;
pub mod types;

// Re-export commonly used items
pub use redis_task_manager::RedisTaskManager;
pub use task_manager::TaskManager;
pub use task_manager_trait::TaskManagerTrait;
pub use server::Server;
pub use server_builder::ServerBuilder;
pub use transport::{Transport, TransportFactory};
pub use transport::nats::{NatsTransport, NatsTransportFactory};
pub use types::{
    AgentCard, JsonRpcRequest, JsonRpcResponse, Message, Part, Task, TaskState, TaskStreamingEvent,
};
