// src/task_manager_trait.rs

use anyhow::Result;
use futures::Stream;
use uuid::Uuid;
use async_trait::async_trait;

use crate::types::{
    AgentCard, Message, Task, TaskError, TaskState, TaskStreamingEvent,
    PushNotificationConfig,
};

/// Trait for task managers to allow for different storage backends
#[async_trait]
pub trait TaskManagerTrait: Send + Sync + 'static {
    /// Get the agent card
    fn agent(&self) -> &AgentCard;
    
    /// Get a task by ID
    async fn get_task(&self, task_id: Uuid) -> Result<Option<Task>>;
    
    /// Create a new task
    async fn create_task(
        &self,
        inputs: Vec<Message>,
        push_notification_config: Option<PushNotificationConfig>,
    ) -> Result<Task>;
    
    /// Update a task
    async fn update_task(&self, task_id: Uuid, state: TaskState, error: Option<TaskError>) -> Result<Task>;
    
    /// Add a message to a task
    async fn add_message(&self, task_id: Uuid, message: Message) -> Result<Task>;
    
    /// Process a task
    async fn process_task(&self, task_id: Uuid) -> Result<Task>;
    
    /// Process a task with streaming events
    async fn process_task_streaming(
        &self,
        task_id: Uuid,
    ) -> Result<Box<dyn Stream<Item = TaskStreamingEvent> + Send + Unpin>>;
    
    /// Cancel a task
    async fn cancel_task(&self, task_id: Uuid) -> Result<Task>;
}
