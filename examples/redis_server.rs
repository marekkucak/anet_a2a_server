// examples/redis_server.rs

use std::time::Duration;
use anyhow::Result;
use log::info;
use uuid::Uuid;

use a2a_framework::{NatsTransportFactory, ServerBuilder, RedisTaskManager, AgentCard};

fn main() -> Result<()> {
    // Initialize logging
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );
    
    info!("Starting A2A server example with Redis persistence");
    
    // Create agent card
    let agent_id = Uuid::new_v4();
    let agent = AgentCard {
        id: agent_id,
        name: "Redis-backed A2A Agent".to_string(),
        description: "An example A2A agent with Redis persistence".to_string(),
        metadata: None,
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
    };
    
    // Create Redis task manager
    // Make sure Redis is running at localhost:6379 first!
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
