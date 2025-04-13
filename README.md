# A2A Framework

A Rust implementation of the Agent-to-Agent (A2A) protocol, designed for building agent services that can communicate with each other using a task-based approach.

## Overview

The A2A Framework provides a foundation for implementing agent services that can:
- Expose agent capabilities and information
- Accept tasks from clients
- Process tasks asynchronously
- Stream task state changes and output back to clients
- Support both request-reply and streaming interaction patterns
- Store task data with in-memory or Redis persistence

The framework is built with a modular architecture, allowing for different transport mechanisms and storage backends, with NATS for transport and optional Redis for persistence.

## Architecture

The framework consists of several key components:

- **Types**: Core data structures representing the A2A protocol (Task, Message, Part, etc.)
- **TaskManagerTrait**: Interface defining task management operations
- **TaskManager**: In-memory implementation of task management
- **RedisTaskManager**: Redis-backed implementation for task persistence
- **Transport**: Abstraction for communication (currently using NATS)
- **Server**: Orchestrates request handling and delegates to the task manager
- **ServerBuilder**: Provides a clean API for configuring and creating a Server

## Getting Started

### Prerequisites

- Rust 1.56 or later
- NATS server (for the default transport)
- Redis server (optional, for persistent storage)

### Installation

Add the dependency to your Cargo.toml:

```toml
[dependencies]
a2a-framework = "0.1.0"
```

### Basic Server Example (In-Memory)

```rust
use std::time::Duration;
use anyhow::Result;
use tokio::runtime::Runtime;
use uuid::Uuid;
use a2a_framework::{NatsTransportFactory, ServerBuilder};

fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();
    
    // Create server with NATS transport and in-memory task manager
    let server = ServerBuilder::new()
        .with_agent_name("Example Agent")
        .with_agent_description("An example A2A agent")
        .with_transport_factory(NatsTransportFactory::new(
            "nats://localhost:4222",
            "a2a.agent",
            Duration::from_secs(5),
        ))
        .build()?;
    
    // Run server until shutdown signal (Ctrl+C)
    server.run_until_shutdown()?;
    
    Ok(())
}
```

### Redis-Backed Server Example

```rust
use std::time::Duration;
use anyhow::Result;
use uuid::Uuid;
use a2a_framework::{
    NatsTransportFactory, 
    ServerBuilder, 
    RedisTaskManager,
    AgentCard
};

fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();
    
    // Create agent card
    let agent = AgentCard {
        id: Uuid::new_v4(),
        name: "Redis-backed A2A Agent".to_string(),
        description: "An example A2A agent with Redis persistence".to_string(),
        metadata: None,
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
    };
    
    // Create Redis task manager
    let task_manager = RedisTaskManager::new(
        "redis://localhost:6379", 
        agent
    )?;
    
    // Create tokio runtime
    let runtime = tokio::runtime::Runtime::new()?;
    
    // Create server with NATS transport and Redis task manager
    let server = ServerBuilder::new()
        .with_task_manager(task_manager)
        .with_transport_factory(NatsTransportFactory::new(
            "nats://localhost:4222",
            "a2a.agent",
            Duration::from_secs(5),
        ))
        .with_runtime(runtime)
        .build()?;
    
    // Run server until shutdown signal
    server.run_until_shutdown()?;
    
    Ok(())
}
```

### Client Example

See the `examples/test_client.rs` file for a complete example of a client that interacts with an A2A server.

## Protocol Methods

The A2A framework implements the following protocol methods:

- `agents/getInfo`: Get information about the agent
- `tasks/create`: Create a new task
- `tasks/get`: Get a task by ID
- `tasks/send`: Process a task and get the result
- `tasks/sendSubscribe`: Process a task with streaming events
- `tasks/cancel`: Cancel a task

## Features

- **Task Processing**: Create, retrieve, process, and cancel tasks
- **Streaming**: Support for streaming task events (state changes, message updates)
- **Multiple Storage Options**: 
  - In-memory task storage for simplicity
  - Redis-backed storage for persistence
- **NATS Transport**: Implementation using NATS for communication
- **Error Handling**: Consistent error mapping to JSON-RPC responses
- **Extensible Design**: Trait-based approach for plugging in different components

## Storage Backends

The framework currently supports two storage backends:

1. **In-Memory Storage** (default)
   - Simple and fast
   - No persistence across restarts
   - Suitable for development and testing

2. **Redis Storage**
   - Persists tasks across server restarts
   - Suitable for production use
   - Configurable cleanup of old tasks
   - Uses JSON serialization for task storage

## Future Enhancements

- **Additional Persistence Options**: Add support for SQL and document databases
- **Input Required Flow**: Support for tasks that require additional input
- **Push Notifications**: Complete implementation of HTTP POST notifications for task updates
- **Additional Transports**: Support for HTTP, WebSockets, etc.
- **Authentication**: Add authentication and authorization mechanisms
- **Metrics and Monitoring**: Add support for collecting and exposing metrics

## License

This project is licensed under the MIT License - see the LICENSE file for details.
