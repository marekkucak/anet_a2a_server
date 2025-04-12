# A2A Framework Architecture

This document outlines the architecture of the A2A Framework, describing its components, patterns, and design decisions.

## High-Level Architecture

The A2A Framework is structured around these main components:

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│                 │     │                 │     │                 │
│  Transport      │◄────┤     Server      │◄────┤  Task Manager   │
│  (NATS)         │     │                 │     │                 │
│                 │     │                 │     │                 │
└─────────────────┘     └─────────────────┘     └─────────────────┘
         ▲                       ▲                      ▲
         │                       │                      │
         │                       │                      │
         │                       │                      │
         └───────────────────────┼──────────────────────┘
                                 │
                         ┌───────┴───────┐
                         │               │
                         │   Types       │
                         │               │
                         └───────────────┘
```

### Components

1. **Types (`src/types.rs`)**: Core data structures representing the A2A protocol concepts.
   - `AgentCard`: Represents an agent and its capabilities
   - `Task`: Represents a task with its state, inputs, outputs
   - `Message`: Represents a message within a task
   - `Part`: Represents a part of a message (text or artifact)
   - `TaskStreamingEvent`: Represents events for streaming task updates

2. **TaskManager (`src/task_manager.rs`)**: Manages the lifecycle of tasks.
   - Task creation, retrieval, updating
   - Task processing (simulated in the current implementation)
   - Streaming event generation for task updates

3. **Transport (`src/transport/`)**: Abstracts the communication mechanism.
   - Transport trait defining common interface
   - NATS implementation for request-reply and streaming
   - Extensible for other transports in the future

4. **Server (`src/server.rs`)**: Orchestrates handling of A2A protocol requests.
   - Method dispatching based on JSON-RPC method
   - Error handling and response generation
   - Delegation to TaskManager for actual task processing

5. **ServerBuilder (`src/server_builder.rs`)**: Provides a builder pattern for creating server instances.
   - Fluent API for configuration
   - Default values where appropriate
   - Validation of required parameters

## Key Design Patterns

1. **Builder Pattern**: Used in `ServerBuilder` to provide a clean, fluent API for creating and configuring a server.

2. **Trait Abstraction**: The `Transport` trait abstracts the communication mechanism, allowing for different implementations.

3. **Factory Pattern**: `TransportFactory` provides a way to create transport instances without exposing implementation details.

4. **Async/Await**: Leveraging Rust's async/await for non-blocking I/O operations.

5. **Event Streaming**: Using Rust's `Stream` trait for modelling event streams.

6. **JSON-RPC Protocol**: Using a standardized RPC protocol for agent communication.

## Transport Design

The transport layer is designed with extensibility in mind:

```
┌───────────────────────────────────────┐
│              Transport Trait          │
└───────────────────────────────────────┘
                    ▲
                    │
        ┌───────────┴────────────┐
        │                        │
┌───────┴───────────┐    ┌───────┴───────────┐
│   NATS Transport   │    │  Future Transport │
└───────────────────┘    └───────────────────┘
```

- The `Transport` trait defines the core operations:
  - `start()`: Initialize connections
  - `stop()`: Gracefully shutdown
  - `run()`: Process incoming requests
  - `publish_event()`: Publish streaming events

- The current NATS implementation:
  - Uses async-nats for communication
  - Implements request-reply pattern
  - Handles streaming by publishing events to reply subjects

## Task Processing Flow

1. **Task Creation**:
   - Client sends `tasks/create` request
   - Server creates a new task with `Created` state
   - TaskManager stores the task

2. **Task Processing (Regular)**:
   - Client sends `tasks/send` request
   - Server changes task state to `Processing`
   - TaskManager processes the task (simulated in current implementation)
   - Server changes task state to `Completed` or `Failed`
   - Final result returned to client

3. **Task Processing (Streaming)**:
   - Client sends `tasks/sendSubscribe` request
   - Server changes task state to `Processing`
   - Server streams state changes and messages back to client
   - Client consumes the stream of events
   - Final state (`Completed` or `Failed`) indicated in the stream

## Future Directions

1. **Persistence Layer**:
   - Add a trait for task storage
   - Implement database-backed storage

2. **Input Required Flow**:
   - Add support for tasks that need additional input
   - Implement state transition to `InputRequired`
   - Add client handling for providing additional input

3. **Push Notifications**:
   - Implement HTTP POST notifications for task updates
   - Support configurable notification endpoints

4. **Error Handling Improvements**:
   - More specific error types
   - Better mapping to JSON-RPC error codes

5. **Testing**:
   - Unit tests for core components
   - Integration tests with mock transports
   - End-to-end tests with real NATS server
