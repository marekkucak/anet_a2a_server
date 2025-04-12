# A2A Framework

A Rust implementation of the Agent-to-Agent (A2A) protocol, designed for building agent services that can communicate with each other using a task-based approach.

## Overview

The A2A Framework provides a foundation for implementing agent services that can:

- Expose agent capabilities and information
- Accept tasks from clients
- Process tasks asynchronously
- Stream task state changes and output back to clients
- Support both request-reply and streaming interaction patterns

The framework is built with a modular architecture, allowing for different transport mechanisms, though currently, it provides a NATS implementation.

## Architecture

The framework consists of several key components:

- **Types**: Core data structures representing the A2A protocol (Task, Message, Part, etc.)
- **Transport**: Abstraction for communication (currently using NATS)
- **TaskManager**: Handles task lifecycle and processing
- **Server**: Orchestrates request handling and delegates to the TaskManager
- **ServerBuilder**: Provides a clean API for configuring and creating a Server

## Getting Started

### Prerequisites

- Rust 1.56 or later
- NATS server (for the default transport)

### Installation

Add the dependency to your Cargo.toml:

```toml
[dependencies]
a2a-framework = "0.1.0"
```

### Basic Server Example

```rust
use std::time::Duration;
use anyhow::Result;
use tokio::runtime::Runtime;
use uuid::Uuid;

use a2a_framework::{NatsTransportFactory, ServerBuilder};

fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();
    
    // Create server with NATS transport
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
- **In-memory Storage**: Basic in-memory task storage (can be extended for persistence)
- **NATS Transport**: Implementation using NATS for communication
- **Error Handling**: Consistent error mapping to JSON-RPC responses

## Future Enhancements

- **Persistence**: Add database storage for tasks
- **Input Required Flow**: Support for tasks that require additional input
- **Push Notifications**: Implement HTTP POST notifications for task updates
- **Additional Transports**: Support for HTTP, WebSockets, etc.
- **Authentication**: Add authentication and authorization mechanisms

## License

This project is licensed under the MIT License - see the LICENSE file for details.
