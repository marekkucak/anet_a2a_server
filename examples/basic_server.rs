use std::time::Duration;
use anyhow::Result;
use log::info;
use tokio::runtime::Runtime;
use uuid::Uuid;

use a2a_framework::{NatsTransportFactory, ServerBuilder};

fn main() -> Result<()> {
    // Initialize logging
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );
    
    info!("Starting A2A server example");
    
    // Create tokio runtime
    let runtime = Runtime::new()?;
    
    // Create server with NATS transport
    let server = ServerBuilder::new()
        .with_agent_id(Uuid::new_v4())
        .with_agent_name("Example A2A Agent")
        .with_agent_description("An example A2A agent implementation")
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
