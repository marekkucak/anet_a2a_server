# A2A Framework Architecture

This document outlines the architecture of the A2A Framework, describing its components, patterns, and design decisions.

## High-Level Architecture

The A2A Framework is structured around these main components:

```
┌─────────────────┐     ┌─────────────────┐     ┌───────────────────────────┐
│                 │     │                 │     │     TaskManagerTrait      │
│  Transport      │◄────┤     Server      │◄────┤                           │
│  (NATS)         │     │                 │     │                           │
│                 │     │                 │     └───────────────────────────┘
└─────────────────┘     └─────────────────┘                  ▲
         ▲                       ▲                           │
         │                       │                  ┌────────┴───────┐
         │                       │                  │                │
         │                       │        ┌─────────┴────┐  ┌────────┴───────┐
         └───────────────────────┼────────┤              │  │                │
                                 │        │ TaskManager  │  │ RedisTaskManager│
                                 │        │ (In-Memory)  │  │                │
                         ┌───────┴───────┐│              │  │                │
                         │               │└──────────────┘  └────────────────┘
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

2. **TaskManagerTrait (`src/task_manager_trait.rs`)**: Interface for task management implementations.
   - Defines methods for task operations
   - Allows for different implementations (in-memory, Redis, etc.)

3. **TaskManager (`src/task_manager.rs`)**: In-memory implementation of TaskManagerTrait.
   - Task creation, retrieval, updating
   - Task processing (simulated in the current implementation)
   - Streaming event generation for task updates

4. **RedisTaskManager (`src/redis_task_manager.rs`)**: Redis-backed implementation of TaskManagerTrait.
   - Provides persistence via Redis
   - Same interface as in-memory TaskManager
   - Handles serialization/deserialization of tasks to/from JSON

5. **Transport (`src/transport/`)**: Abstracts the communication mechanism.
   - Transport trait defining common interface
   - NATS implementation for request-reply and streaming
   - Extensible for other transports in the future

6. **Server (`src/server.rs`)**: Orchestrates handling of A2A protocol requests.
   - Method dispatching based on JSON-RPC method
   - Error handling and response generation
   - Delegation to TaskManager for actual task processing

7. **ServerBuilder (`src/server_builder.rs`)**: Provides a builder pattern for creating server instances.
   - Fluent API for configuration
   - Default values where appropriate
   - Validation of required parameters
   - Support for configuring different task manager implementations

## Key Design Patterns

1. **Builder Pattern**: Used in `ServerBuilder` to provide a clean, fluent API for creating and configuring a server.

2. **Trait Abstraction**: 
   - The `Transport` trait abstracts the communication mechanism, allowing for different implementations.
   - The `TaskManagerTrait` abstracts task storage and processing, enabling multiple backends.

3. **Factory Pattern**: `TransportFactory` provides a way to create transport instances without exposing implementation details.

4. **Async/Await**: Leveraging Rust's async/await for non-blocking I/O operations.

5. **Event Streaming**: Using Rust's `Stream` trait for modelling event streams.

6. **JSON-RPC Protocol**: Using a standardized RPC protocol for agent communication.

7. **Repository Pattern**: TaskManager implementations follow the repository pattern for task storage and retrieval.

## TaskManager Design

The task management layer is designed with a clear abstraction through the `TaskManagerTrait`:

```
┌───────────────────────────────────────┐
│            TaskManagerTrait           │
└───────────────────────────────────────┘
                    ▲
                    │
        ┌───────────┴────────────┐
        │                        │
┌───────┴───────────┐    ┌───────┴───────────┐
│    TaskManager    │    │  RedisTaskManager │
│    (In-Memory)    │    │                   │
└───────────────────┘    └───────────────────┘
```

- The `TaskManagerTrait` defines core operations:
  - `agent()`: Get the agent card
  - `get_task()`: Retrieve a task by ID
  - `create_task()`: Create a new task
  - `update_task()`: Update a task's state
  - `add_message()`: Add a message to a task
  - `process_task()`: Process a task
  - `process_task_streaming()`: Process a task with streaming events
  - `cancel_task()`: Cancel a task

- The `TaskManager` implementation uses in-memory storage
- The `RedisTaskManager` implementation uses Redis for persistence

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

## Persistence with Redis

The `RedisTaskManager` provides task persistence using Redis:

1. **Storage Format**:
   - Tasks are stored as JSON strings
   - Keys are formatted as `task:{task_id}`
   - Task attributes stored as Redis key/value pairs

2. **Operations**:
   - Create tasks in Redis
   - Read tasks from Redis
   - Update task state and content
   - Delete/cleanup old tasks
   - Stream task updates while updating Redis

3. **Key Features**:
   - Compatible with the TaskManagerTrait interface
   - Provides persistence across restarts
   - Allows for cleanup of old tasks

## Future Directions

1. **Additional Persistence Options**:
   - Implement SQL database backends
   - Add support for document databases

2. **Input Required Flow**:
   - Add support for tasks that need additional input
   - Implement state transition to `InputRequired`
   - Add client handling for providing additional input

3. **Push Notifications**:
   - Complete the implementation of HTTP POST notifications for task updates
   - Support configurable notification endpoints

4. **Error Handling Improvements**:
   - More specific error types
   - Better mapping to JSON-RPC error codes

5. **Testing**:
   - Unit tests for core components
   - Integration tests with mock transports
   - End-to-end tests with real NATS server and Redis
