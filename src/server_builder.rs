// src/server_builder.rs

use std::sync::Arc;
use anyhow::Result;
use tokio::runtime::Runtime;
use uuid::Uuid;

use crate::task_manager::TaskManager;
use crate::task_manager_trait::TaskManagerTrait; // Add this import
use crate::server::Server;
use crate::transport::TransportFactory;
use crate::types::AgentCard;

/// Builder for Server
pub struct ServerBuilder {
    task_manager: Option<Arc<dyn TaskManagerTrait>>, // Changed from Option<Arc<TaskManager>>
    agent_id: Option<Uuid>,
    agent_name: Option<String>,
    agent_description: Option<String>,
    transport_factory: Option<Box<dyn TransportFactory>>,
    runtime: Option<Runtime>,
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerBuilder {
    /// Create a new ServerBuilder
    pub fn new() -> Self {
        Self {
            task_manager: None,
            agent_id: None,
            agent_name: None,
            agent_description: None,
            transport_factory: None,
            runtime: None,
        }
    }
    
    /// Set a custom task manager (like RedisTaskManager)
    pub fn with_task_manager<T>(mut self, task_manager: T) -> Self 
    where 
        T: TaskManagerTrait + 'static 
    {
        self.task_manager = Some(Arc::new(task_manager));
        self
    }
    
    /// Set the agent ID
    pub fn with_agent_id(mut self, agent_id: Uuid) -> Self {
        self.agent_id = Some(agent_id);
        self
    }
    
    /// Set the agent name
    pub fn with_agent_name(mut self, name: impl Into<String>) -> Self {
        self.agent_name = Some(name.into());
        self
    }
    
    /// Set the agent description
    pub fn with_agent_description(mut self, description: impl Into<String>) -> Self {
        self.agent_description = Some(description.into());
        self
    }
    
    /// Set the transport factory
    pub fn with_transport_factory(mut self, factory: impl TransportFactory + 'static) -> Self {
        self.transport_factory = Some(Box::new(factory));
        self
    }
    
    /// Set the runtime
    pub fn with_runtime(mut self, runtime: Runtime) -> Self {
        self.runtime = Some(runtime);
        self
    }
    
    /// Build the server
    pub fn build(self) -> Result<Server> {
        // Extract transport factory or error
        let transport_factory = self
            .transport_factory
            .ok_or_else(|| anyhow::anyhow!("Transport factory is required"))?;
        
        // Extract or create runtime
        let runtime = self.runtime.unwrap_or_else(|| {
            Runtime::new().expect("Failed to create Tokio runtime")
        });
        
        // Use provided task manager or create a default one
        let task_manager = if let Some(tm) = self.task_manager {
            tm
        } else {
            // Extract or generate agent ID
            let agent_id = self.agent_id.unwrap_or_else(Uuid::new_v4);
            
            // Extract or default agent name
            let agent_name = self.agent_name.unwrap_or_else(|| "A2A Agent".to_string());
            
            // Extract or default agent description
            let agent_description = self.agent_description.unwrap_or_else(|| {
                "A generic A2A agent implementation".to_string()
            });
            
            // Create agent card
            let agent = AgentCard {
                id: agent_id,
                name: agent_name,
                description: agent_description,
                metadata: None,
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            };
            
            // Create default task manager
            Arc::new(TaskManager::new(agent))
        };
        
        // Create transport
        let transport = transport_factory.create()?;
        
        // Create server using the constructor
        let server = Server::new(task_manager, transport, runtime);
        
        Ok(server)
    }
}
