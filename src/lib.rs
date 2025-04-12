//! A Rust implementation of the A2A (Agent-to-Agent) protocol.
//!
//! This library provides a framework for building A2A agents, which can communicate
//! with each other using the A2A protocol. The protocol is designed to facilitate
//! task-based communication between agents, where one agent (the client) can send
//! tasks to another agent (the server) for processing.
//!
//! The framework is modular, with components for:
//! - Task management
//! - Transport (NATS implementation provided)
//! - Server handling

pub mod server;
pub mod server_builder;
pub mod task_manager;
pub mod transport;
pub mod types;

// Re-export commonly used items
pub use server::Server;
pub use server_builder::ServerBuilder;
pub use task_manager::TaskManager;
pub use transport::{Transport, TransportFactory};
pub use transport::nats::{NatsTransport, NatsTransportFactory};

// Re-export key types
pub use types::{
    AgentCard, JsonRpcRequest, JsonRpcResponse, Message, Part, Task, TaskState, TaskStreamingEvent,
};
