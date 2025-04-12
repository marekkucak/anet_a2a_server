use std::collections::HashMap;
use anyhow::{Context, Result};
use chrono::Utc;
use futures::{stream, Stream, StreamExt};
use log::{debug, info, warn};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::types::{
    AgentCard, Message, Part, Task, TaskError, TaskState, TaskStreamingEvent,
    PushNotificationConfig,
};

/// Manager for handling tasks
pub struct TaskManager {
    tasks: Mutex<HashMap<Uuid, Task>>,
    agent: AgentCard,
}

impl TaskManager {
    /// Create a new TaskManager
    pub fn new(agent: AgentCard) -> Self {
        Self {
            tasks: Mutex::new(HashMap::new()),
            agent,
        }
    }
    
    /// Get the agent card
    pub fn agent(&self) -> &AgentCard {
        &self.agent
    }
    
    /// Get a task by ID
    pub async fn get_task(&self, task_id: Uuid) -> Result<Option<Task>> {
        let tasks = self.tasks.lock().await;
        Ok(tasks.get(&task_id).cloned())
    }
    
    /// Create a new task
    pub async fn create_task(
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
        
        let mut tasks = self.tasks.lock().await;
        tasks.insert(task_id, task.clone());
        
        info!("Created task {}", task_id);
        Ok(task)
    }
    
    /// Update a task
    pub async fn update_task(&self, task_id: Uuid, state: TaskState, error: Option<TaskError>) -> Result<Task> {
        let mut tasks = self.tasks.lock().await;
        
        let task = tasks.get_mut(&task_id).context("Task not found")?;
        task.state = state.clone(); // Clone state before moving it
        task.error = error;
        task.updated_at = Some(Utc::now());
        
        info!("Updated task {} to state {:?}", task_id, state);
        Ok(task.clone())
    }
    
    /// Add a message to a task
    pub async fn add_message(&self, task_id: Uuid, message: Message) -> Result<Task> {
        let mut tasks = self.tasks.lock().await;
        
        let task = tasks.get_mut(&task_id).context("Task not found")?;
        if message.role == "assistant" {
            task.outputs.push(message);
        } else {
            task.inputs.push(message);
        }
        task.updated_at = Some(Utc::now());
        
        info!("Added message to task {}", task_id);
        Ok(task.clone())
    }
    
    /// Process a task (simulated)
    pub async fn process_task(&self, task_id: Uuid) -> Result<Task> {
        // Simulate task processing
        let task = {
            self.update_task(task_id, TaskState::Processing, None).await?
        };
        
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
    pub async fn process_task_streaming(
        &self,
        task_id: Uuid,
    ) -> Result<impl Stream<Item = TaskStreamingEvent>> {
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
        
        // Create actual stream with delays
        let task_manager = self.clone();
        let stream = stream::iter(events)
            .then(move |event| {
                let task_manager = task_manager.clone();
                let task_id = task_id;
                
                async move {
                    // Add a delay to simulate processing time
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    
                    // Update the task state in storage based on events
                    match &event {
                        TaskStreamingEvent::StateChange { state, error, .. } => {
                            task_manager
                                .update_task(task_id, state.clone(), error.clone())
                                .await
                                .unwrap_or_else(|e| {
                                    warn!("Failed to update task state: {}", e);
                                    // Return a default task if update fails
                                    Task {
                                        id: task_id,
                                        agent_id: task_manager.agent.id,
                                        created_at: Utc::now(),
                                        updated_at: Some(Utc::now()),
                                        state: state.clone(),
                                        inputs: Vec::new(),
                                        outputs: Vec::new(),
                                        error: error.clone(),
                                        push_notification_config: None,
                                    }
                                });
                        }
                        TaskStreamingEvent::MessageAdd { message, .. } => {
                            task_manager
                                .add_message(task_id, message.clone())
                                .await
                                .unwrap_or_else(|e| {
                                    warn!("Failed to add message: {}", e);
                                    // Return a default task if update fails
                                    Task {
                                        id: task_id,
                                        agent_id: task_manager.agent.id,
                                        created_at: Utc::now(),
                                        updated_at: Some(Utc::now()),
                                        state: TaskState::Processing,
                                        inputs: Vec::new(),
                                        outputs: vec![message.clone()],
                                        error: None,
                                        push_notification_config: None,
                                    }
                                });
                        }
                    }
                    
                    event
                }
            });
        
        Ok(stream)
    }
    
    /// Cancel a task
    pub async fn cancel_task(&self, task_id: Uuid) -> Result<Task> {
        let current_task = {
            let tasks = self.tasks.lock().await;
            tasks.get(&task_id).cloned().context("Task not found")?
        };
        
        // Check if task can be cancelled
        match current_task.state {
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled => {
                return Err(anyhow::anyhow!(
                    "Cannot cancel task in state {:?}",
                    current_task.state
                ));
            }
            _ => {}
        }
        
        // Update task state to cancelled
        self.update_task(task_id, TaskState::Cancelled, None).await
    }
}

impl Clone for TaskManager {
    fn clone(&self) -> Self {
        Self {
            tasks: Mutex::new(HashMap::new()),
            agent: self.agent.clone(),
        }
    }
}
