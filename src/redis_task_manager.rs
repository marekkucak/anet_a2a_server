// src/redis_task_manager.rs

use anyhow::{Context, Result};
use chrono::Utc;
use futures::{stream, Stream, StreamExt};
use log::{debug, info, warn};
use redis::{AsyncCommands, Client};
use serde_json;
use uuid::Uuid;
use async_trait::async_trait;

use crate::types::{
    AgentCard, Message, Part, Task, TaskError, TaskState, TaskStreamingEvent,
    PushNotificationConfig,
};
use crate::task_manager_trait::TaskManagerTrait;

/// Manager for handling tasks with Redis persistence
pub struct RedisTaskManager {
    client: Client,
    agent: AgentCard,
}

impl RedisTaskManager {
    /// Create a new RedisTaskManager
    pub fn new(redis_url: &str, agent: AgentCard) -> Result<Self> {
        let client = Client::open(redis_url)?;
        
        Ok(Self {
            client,
            agent,
        })
    }
    
    /// Get a Redis connection from the pool
    async fn get_conn(&self) -> Result<redis::aio::Connection> {
        let conn = self.client.get_async_connection().await?;
        Ok(conn)
    }
    
    /// Task key in Redis
    fn task_key(task_id: &Uuid) -> String {
        format!("task:{}", task_id)
    }
    
    /// Cleanup old tasks (optional, good for maintenance)
    pub async fn cleanup_old_tasks(&self, max_age_days: i64) -> Result<usize> {
        let mut conn = self.get_conn().await?;
        
        // Find all task keys
        let task_keys: Vec<String> = conn.keys("task:*").await?;
        let mut deleted_count = 0;
        
        let cutoff_time = Utc::now() - chrono::Duration::days(max_age_days);
        
        for key in task_keys {
            let task_json: String = conn.get(&key).await?;
            let task: Task = match serde_json::from_str(&task_json) {
                Ok(t) => t,
                Err(e) => {
                    warn!("Failed to deserialize task {}: {}", key, e);
                    continue;
                }
            };
            
            if task.created_at < cutoff_time {
                // Explicitly specify the return type to avoid the never type fallback issue
                let _: () = conn.del(&key).await?;
                deleted_count += 1;
                debug!("Deleted old task {}", task.id);
            }
        }
        
        info!("Cleaned up {} old tasks", deleted_count);
        Ok(deleted_count)
    }
}

#[async_trait]
impl TaskManagerTrait for RedisTaskManager {
    /// Get the agent card
    fn agent(&self) -> &AgentCard {
        &self.agent
    }
    
    /// Get a task by ID
    async fn get_task(&self, task_id: Uuid) -> Result<Option<Task>> {
        let mut conn = self.get_conn().await?;
        
        // Check if task exists
        let exists: bool = conn.exists(Self::task_key(&task_id)).await?;
        if !exists {
            return Ok(None);
        }
        
        // Get task JSON
        let task_json: String = conn.get(Self::task_key(&task_id)).await?;
        
        // Deserialize task
        let task: Task = serde_json::from_str(&task_json)
            .context("Failed to deserialize task")?;
        
        Ok(Some(task))
    }
    
    /// Create a new task
    async fn create_task(
        &self,
        inputs: Vec<Message>,
        push_notification_config: Option<PushNotificationConfig>,
    ) -> Result<Task> {
        let task_id = Uuid::new_v4();
        let now = Utc::now();
        
        let task = Task {
            id: task_id,
            agent_id: self.agent.id,
            created_at: now,
            updated_at: Some(now),
            state: TaskState::Created,
            inputs,
            outputs: Vec::new(),
            error: None,
            push_notification_config,
        };
        
        // Serialize and save task
        let task_json = serde_json::to_string(&task)?;
        let mut conn = self.get_conn().await?;
        // Explicitly specify the return type to avoid the never type fallback issue
        let _: () = conn.set(Self::task_key(&task_id), task_json).await?;
        
        info!("Created task {}", task_id);
        Ok(task)
    }
    
    /// Update a task
    async fn update_task(&self, task_id: Uuid, state: TaskState, error: Option<TaskError>) -> Result<Task> {
        let mut conn = self.get_conn().await?;
        
        // Get current task
        let exists: bool = conn.exists(Self::task_key(&task_id)).await?;
        if !exists {
            return Err(anyhow::anyhow!("Task not found"));
        }
        
        let task_json: String = conn.get(Self::task_key(&task_id)).await?;
        let mut task: Task = serde_json::from_str(&task_json)
            .context("Failed to deserialize task")?;
        
        // Update task
        task.state = state;
        task.error = error;
        task.updated_at = Some(Utc::now());
        
        // Save updated task
        let updated_task_json = serde_json::to_string(&task)?;
        // Explicitly specify the return type to avoid the never type fallback issue
        let _: () = conn.set(Self::task_key(&task_id), updated_task_json).await?;
        
        info!("Updated task {} to state {:?}", task_id, task.state);
        Ok(task)
    }
    
    /// Add a message to a task
    async fn add_message(&self, task_id: Uuid, message: Message) -> Result<Task> {
        let mut conn = self.get_conn().await?;
        
        // Get current task
        let exists: bool = conn.exists(Self::task_key(&task_id)).await?;
        if !exists {
            return Err(anyhow::anyhow!("Task not found"));
        }
        
        let task_json: String = conn.get(Self::task_key(&task_id)).await?;
        let mut task: Task = serde_json::from_str(&task_json)
            .context("Failed to deserialize task")?;
        
        // Add message
        if message.role == "assistant" {
            task.outputs.push(message);
        } else {
            task.inputs.push(message);
        }
        task.updated_at = Some(Utc::now());
        
        // Save updated task
        let updated_task_json = serde_json::to_string(&task)?;
        // Explicitly specify the return type to avoid the never type fallback issue
        let _: () = conn.set(Self::task_key(&task_id), updated_task_json).await?;
        
        info!("Added message to task {}", task_id);
        Ok(task)
    }
    
    /// Process a task (simulated)
    async fn process_task(&self, task_id: Uuid) -> Result<Task> {
        // Update task state to Processing
        self.update_task(task_id, TaskState::Processing, None).await?;
        
        // In a real implementation, this would run the actual task processing logic
        // For this example, we'll just simulate success after a delay
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        // Create a response message
        let message = Message {
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            role: "assistant".to_string(),
            parts: vec![Part::Text {
                text: "This is a simulated response from the agent.".to_string(),
            }],
        };
        
        self.add_message(task_id, message).await?;
        
        // Get the task to check for push notification config
        let task = self.get_task(task_id).await?
            .context("Task not found after processing")?;
        
        // If task has push_notification_config, send a notification
        // This would be implemented in a real system
        if let Some(config) = &task.push_notification_config {
            debug!("Would send push notification to {}", config.url);
            // In a real implementation:
            // let client = reqwest::Client::new();
            // client.post(&config.url)
            //     .json(&json!({ "taskId": task_id, "state": "completed" }))
            //     .send()
            //     .await?;
        }
        
        // Mark task as completed
        self.update_task(task_id, TaskState::Completed, None).await
    }
    
    /// Process a task with streaming events
    async fn process_task_streaming(
        &self,
        task_id: Uuid,
    ) -> Result<Box<dyn Stream<Item = TaskStreamingEvent> + Send + Unpin>> {
        // Update the task state to Processing
        self.update_task(task_id, TaskState::Processing, None).await?;
        
        // Create a stream of events for this task
        let events = vec![
            // State change to processing
            TaskStreamingEvent::StateChange {
                event: "task_state_changed".to_string(),
                task_id,
                state: TaskState::Processing,
                error: None,
            },
            
            // Simulated thinking process as a message
            TaskStreamingEvent::MessageAdd {
                event: "message_added".to_string(),
                task_id,
                message: Message {
                    id: Uuid::new_v4(),
                    created_at: Utc::now(),
                    role: "assistant".to_string(),
                    parts: vec![Part::Text {
                        text: "I'm thinking about this...".to_string(),
                    }],
                },
            },
            
            // Simulated response as a message
            TaskStreamingEvent::MessageAdd {
                event: "message_added".to_string(),
                task_id,
                message: Message {
                    id: Uuid::new_v4(),
                    created_at: Utc::now(),
                    role: "assistant".to_string(),
                    parts: vec![Part::Text {
                        text: "This is a simulated response from the agent.".to_string(),
                    }],
                },
            },
            
            // State change to completed
            TaskStreamingEvent::StateChange {
                event: "task_state_changed".to_string(),
                task_id,
                state: TaskState::Completed,
                error: None,
            },
        ];
        
        // Create a copy of self for the stream closure
        let task_manager = self.clone();
        
        // Create the stream and wrap in Box to match trait return type
        let stream = stream::iter(events)
            .then(move |event| {
                let task_manager = task_manager.clone();
                let task_id = task_id;
                
                async move {
                    // Add a delay to simulate processing time
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    
                    // Update the task state in Redis based on events
                    match &event {
                        TaskStreamingEvent::StateChange { state, error, .. } => {
                            match task_manager.update_task(task_id, state.clone(), error.clone()).await {
                                Ok(_) => {},
                                Err(e) => warn!("Failed to update task state: {}", e),
                            }
                        }
                        TaskStreamingEvent::MessageAdd { message, .. } => {
                            match task_manager.add_message(task_id, message.clone()).await {
                                Ok(_) => {},
                                Err(e) => warn!("Failed to add message: {}", e),
                            }
                        }
                    }
                    
                    event
                }
            });
            
        // First pin and then box
        let boxed_stream: Box<dyn Stream<Item = TaskStreamingEvent> + Send + Unpin> = 
            Box::new(Box::pin(stream));
            
        Ok(boxed_stream)
    }
    
    /// Cancel a task
    async fn cancel_task(&self, task_id: Uuid) -> Result<Task> {
        // Get current task
        let task = self.get_task(task_id).await?
            .context("Task not found")?;
        
        // Check if task can be cancelled
        match task.state {
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled => {
                return Err(anyhow::anyhow!(
                    "Cannot cancel task in state {:?}",
                    task.state
                ));
            }
            _ => {}
        }
        
        // Update task state to cancelled
        self.update_task(task_id, TaskState::Cancelled, None).await
    }
}

impl Clone for RedisTaskManager {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            agent: self.agent.clone(),
        }
    }
}
