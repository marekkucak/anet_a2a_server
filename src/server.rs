use std::sync::Arc;
use anyhow::{Context, Result};
use futures::StreamExt;
use log::{debug, error, info};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use tokio::signal::ctrl_c;
use uuid::Uuid;

use crate::task_manager::TaskManager;
use crate::transport::Transport;
use crate::types::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, Message, PushNotificationConfig, TaskState,
};

/// Server for handling A2A requests
pub struct Server {
    task_manager: Arc<TaskManager>,
    transport: Arc<dyn Transport>,
    runtime: Runtime,
}

impl Server {
    /// Create a new Server instance
    pub(crate) fn new(
        task_manager: Arc<TaskManager>,
        transport: Arc<dyn Transport>,
        runtime: Runtime,
    ) -> Self {
        Self {
            task_manager,
            transport,
            runtime,
        }
    }

    /// Handle a JSON-RPC request
    pub async fn handle_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        debug!("Handling request: {:?}", request);
        
        match request.method.as_str() {
            "agents/getInfo" => self.handle_get_info(request).await,
            "tasks/create" => self.handle_create_task(request).await,
            "tasks/get" => self.handle_get_task(request).await,
            "tasks/send" => self.handle_send_task(request).await,
            "tasks/sendSubscribe" => self.handle_send_subscribe_task(request).await,
            "tasks/cancel" => self.handle_cancel_task(request).await,
            _ => Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: "Method not found".to_string(),
                    data: None,
                }),
            }),
        }
    }
    
    /// Handle agents/getInfo request
    async fn handle_get_info(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        // Return the agent information
        Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(json!({
                "agent": self.task_manager.agent(),
            })),
            error: None,
        })
    }
    
    /// Handle tasks/create request
    async fn handle_create_task(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        // Extract parameters
        let params = request.params;
        
        // Extract messages
        let messages = match params.get("messages") {
            Some(Value::Array(messages)) => {
                let mut result = Vec::new();
                for message in messages {
                    let message: Message = serde_json::from_value(message.clone())
                        .context("Invalid message format")?;
                    result.push(message);
                }
                result
            }
            _ => {
                return Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "Invalid params: messages is required and must be an array".to_string(),
                        data: None,
                    }),
                });
            }
        };
        
        // Extract push notification config (optional)
        let push_notification_config = match params.get("pushNotificationConfig") {
            Some(config) => {
                let config: PushNotificationConfig = serde_json::from_value(config.clone())
                    .context("Invalid pushNotificationConfig format")?;
                Some(config)
            }
            None => None,
        };
        
        // Create the task
        let task = self.task_manager
            .create_task(messages, push_notification_config)
            .await
            .context("Failed to create task")?;
        
        // Return the task
        Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(json!({
                "task": task,
            })),
            error: None,
        })
    }
    
    /// Handle tasks/get request
    async fn handle_get_task(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        // Extract parameters
        let params = request.params;
        
        // Extract task ID
        let task_id = match params.get("taskId") {
            Some(Value::String(id_str)) => {
                Uuid::parse_str(id_str).context("Invalid task ID format")?
            }
            _ => {
                return Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "Invalid params: taskId is required and must be a string".to_string(),
                        data: None,
                    }),
                });
            }
        };
        
        // Get the task
        let task = match self.task_manager.get_task(task_id).await? {
            Some(task) => task,
            None => {
                return Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: 404,
                        message: "Task not found".to_string(),
                        data: None,
                    }),
                });
            }
        };
        
        // Return the task
        Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(json!({
                "task": task,
            })),
            error: None,
        })
    }
    
    /// Handle tasks/send request
    async fn handle_send_task(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        // Extract parameters
        let params = request.params;
        
        // Extract task ID
        let task_id = match params.get("taskId") {
            Some(Value::String(id_str)) => {
                Uuid::parse_str(id_str).context("Invalid task ID format")?
            }
            _ => {
                return Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "Invalid params: taskId is required and must be a string".to_string(),
                        data: None,
                    }),
                });
            }
        };
        
        // Check if task exists
        let task = match self.task_manager.get_task(task_id).await? {
            Some(task) => task,
            None => {
                return Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: 404,
                        message: "Task not found".to_string(),
                        data: None,
                    }),
                });
            }
        };
        
        // Check if task is in the right state
        if task.state != TaskState::Created && task.state != TaskState::InputRequired {
            return Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: 400,
                    message: format!(
                        "Task is in state {:?} and cannot be processed",
                        task.state
                    ),
                    data: None,
                }),
            });
        }
        
        // Process the task
        let updated_task = self.task_manager
            .process_task(task_id)
            .await
            .context("Failed to process task")?;
        
        // Return the updated task
        Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(json!({
                "task": updated_task,
            })),
            error: None,
        })
    }
    
    /// Handle tasks/sendSubscribe request
    async fn handle_send_subscribe_task(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        // Extract parameters
        let params = request.params;
        
        // Extract task ID
        let task_id = match params.get("taskId") {
            Some(Value::String(id_str)) => {
                Uuid::parse_str(id_str).context("Invalid task ID format")?
            }
            _ => {
                return Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "Invalid params: taskId is required and must be a string".to_string(),
                        data: None,
                    }),
                });
            }
        };
        
        // Check if task exists
        let task = match self.task_manager.get_task(task_id).await? {
            Some(task) => task,
            None => {
                return Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: 404,
                        message: "Task not found".to_string(),
                        data: None,
                    }),
                });
            }
        };
        
        // Check if task is in the right state
        if task.state != TaskState::Created && task.state != TaskState::InputRequired {
            return Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: 400,
                    message: format!(
                        "Task is in state {:?} and cannot be processed",
                        task.state
                    ),
                    data: None,
                }),
            });
        }
        
        // Process the task with streaming
        let event_stream = self.task_manager
            .process_task_streaming(task_id)
            .await
            .context("Failed to process task with streaming")?;
        
        // Collect events (in production, these would be streamed via transport)
        let mut events = Vec::new();
        tokio::pin!(event_stream);
        while let Some(event) = event_stream.next().await {
            events.push(event);
        }
        
        // Return all events (in a real implementation, these would be streamed)
        Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(json!({
                "events": events,
            })),
            error: None,
        })
    }
    
    /// Handle tasks/cancel request
    async fn handle_cancel_task(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        // Extract parameters
        let params = request.params;
        
        // Extract task ID
        let task_id = match params.get("taskId") {
            Some(Value::String(id_str)) => {
                Uuid::parse_str(id_str).context("Invalid task ID format")?
            }
            _ => {
                return Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "Invalid params: taskId is required and must be a string".to_string(),
                        data: None,
                    }),
                });
            }
        };
        
        // Try to cancel the task
        match self.task_manager.cancel_task(task_id).await {
            Ok(updated_task) => {
                // Return the updated task
                Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(json!({
                        "task": updated_task,
                    })),
                    error: None,
                })
            }
            Err(e) => {
                // Return error
                Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: 400,
                        message: e.to_string(),
                        data: None,
                    }),
                })
            }
        }
    }
    
    /// Start the server
    pub fn start(&self) -> Result<()> {
        info!("Starting server...");
        
        // Start the transport
        let transport_clone = self.transport.clone();
        let runtime_handle = self.runtime.handle().clone();
        
        runtime_handle.spawn(async move {
            if let Err(e) = transport_clone.start().await {
                error!("Failed to start transport: {}", e);
                return;
            }
        });
        
        // Run the transport with a request handler
        let transport = self.transport.clone();
        let task_manager = self.task_manager.clone();
        let runtime_handle = self.runtime.handle().clone();
        
        // Create a handler that uses the task manager directly
        let handler = crate::transport::create_handler(move |request| {
            let tm = task_manager.clone();
            let method = request.method.clone();
            
            async move {
                match method.as_str() {
                    "agents/getInfo" => {
                        Ok(JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: Some(json!({
                                "agent": tm.agent(),
                            })),
                            error: None,
                        })
                    }
                    "tasks/create" => {
                        // Extract parameters
                        let params = request.params;
                        
                        // Extract messages
                        let messages = match params.get("messages") {
                            Some(Value::Array(messages)) => {
                                let mut result = Vec::new();
                                for message in messages {
                                    let message: Message = serde_json::from_value(message.clone())
                                        .context("Invalid message format")?;
                                    result.push(message);
                                }
                                result
                            }
                            _ => {
                                return Ok(JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: request.id,
                                    result: None,
                                    error: Some(JsonRpcError {
                                        code: -32602,
                                        message: "Invalid params: messages is required and must be an array".to_string(),
                                        data: None,
                                    }),
                                });
                            }
                        };
                        
                        // Extract push notification config (optional)
                        let push_notification_config = match params.get("pushNotificationConfig") {
                            Some(config) => {
                                let config: PushNotificationConfig = serde_json::from_value(config.clone())
                                    .context("Invalid pushNotificationConfig format")?;
                                Some(config)
                            }
                            None => None,
                        };
                        
                        // Create the task
                        let task = tm
                            .create_task(messages, push_notification_config)
                            .await
                            .context("Failed to create task")?;
                        
                        // Return the task
                        Ok(JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: Some(json!({
                                "task": task,
                            })),
                            error: None,
                        })
                    }
                    "tasks/get" => {
                        // Extract parameters
                        let params = request.params;
                        
                        // Extract task ID
                        let task_id = match params.get("taskId") {
                            Some(Value::String(id_str)) => {
                                Uuid::parse_str(id_str).context("Invalid task ID format")?
                            }
                            _ => {
                                return Ok(JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: request.id,
                                    result: None,
                                    error: Some(JsonRpcError {
                                        code: -32602,
                                        message: "Invalid params: taskId is required and must be a string".to_string(),
                                        data: None,
                                    }),
                                });
                            }
                        };
                        
                        // Get the task
                        let task = match tm.get_task(task_id).await? {
                            Some(task) => task,
                            None => {
                                return Ok(JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: request.id,
                                    result: None,
                                    error: Some(JsonRpcError {
                                        code: 404,
                                        message: "Task not found".to_string(),
                                        data: None,
                                    }),
                                });
                            }
                        };
                        
                        // Return the task
                        Ok(JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: Some(json!({
                                "task": task,
                            })),
                            error: None,
                        })
                    }
                    "tasks/send" => {
                        // Extract parameters
                        let params = request.params;
                        
                        // Extract task ID
                        let task_id = match params.get("taskId") {
                            Some(Value::String(id_str)) => {
                                Uuid::parse_str(id_str).context("Invalid task ID format")?
                            }
                            _ => {
                                return Ok(JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: request.id,
                                    result: None,
                                    error: Some(JsonRpcError {
                                        code: -32602,
                                        message: "Invalid params: taskId is required and must be a string".to_string(),
                                        data: None,
                                    }),
                                });
                            }
                        };
                        
                        // Check if task exists
                        let task = match tm.get_task(task_id).await? {
                            Some(task) => task,
                            None => {
                                return Ok(JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: request.id,
                                    result: None,
                                    error: Some(JsonRpcError {
                                        code: 404,
                                        message: "Task not found".to_string(),
                                        data: None,
                                    }),
                                });
                            }
                        };
                        
                        // Check if task is in the right state
                        if task.state != TaskState::Created && task.state != TaskState::InputRequired {
                            return Ok(JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: request.id,
                                result: None,
                                error: Some(JsonRpcError {
                                    code: 400,
                                    message: format!(
                                        "Task is in state {:?} and cannot be processed",
                                        task.state
                                    ),
                                    data: None,
                                }),
                            });
                        }
                        
                        // Process the task
                        let updated_task = tm
                            .process_task(task_id)
                            .await
                            .context("Failed to process task")?;
                        
                        // Return the updated task
                        Ok(JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: Some(json!({
                                "task": updated_task,
                            })),
                            error: None,
                        })
                    }
                    "tasks/sendSubscribe" => {
                        // Extract parameters
                        let params = request.params;
                        
                        // Extract task ID
                        let task_id = match params.get("taskId") {
                            Some(Value::String(id_str)) => {
                                Uuid::parse_str(id_str).context("Invalid task ID format")?
                            }
                            _ => {
                                return Ok(JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: request.id,
                                    result: None,
                                    error: Some(JsonRpcError {
                                        code: -32602,
                                        message: "Invalid params: taskId is required and must be a string".to_string(),
                                        data: None,
                                    }),
                                });
                            }
                        };
                        
                        // Check if task exists
                        let task = match tm.get_task(task_id).await? {
                            Some(task) => task,
                            None => {
                                return Ok(JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: request.id,
                                    result: None,
                                    error: Some(JsonRpcError {
                                        code: 404,
                                        message: "Task not found".to_string(),
                                        data: None,
                                    }),
                                });
                            }
                        };
                        
                        // Check if task is in the right state
                        if task.state != TaskState::Created && task.state != TaskState::InputRequired {
                            return Ok(JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: request.id,
                                result: None,
                                error: Some(JsonRpcError {
                                    code: 400,
                                    message: format!(
                                        "Task is in state {:?} and cannot be processed",
                                        task.state
                                    ),
                                    data: None,
                                }),
                            });
                        }
                        
                        // Process the task with streaming
                        let event_stream = tm
                            .process_task_streaming(task_id)
                            .await
                            .context("Failed to process task with streaming")?;
                        
                        // Collect events (in production, these would be streamed via transport)
                        let mut events = Vec::new();
                        tokio::pin!(event_stream);
                        while let Some(event) = event_stream.next().await {
                            events.push(event);
                        }
                        
                        // Return all events (in a real implementation, these would be streamed)
                        Ok(JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: Some(json!({
                                "events": events,
                            })),
                            error: None,
                        })
                    }
                    "tasks/cancel" => {
                        // Extract parameters
                        let params = request.params;
                        
                        // Extract task ID
                        let task_id = match params.get("taskId") {
                            Some(Value::String(id_str)) => {
                                Uuid::parse_str(id_str).context("Invalid task ID format")?
                            }
                            _ => {
                                return Ok(JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: request.id,
                                    result: None,
                                    error: Some(JsonRpcError {
                                        code: -32602,
                                        message: "Invalid params: taskId is required and must be a string".to_string(),
                                        data: None,
                                    }),
                                });
                            }
                        };
                        
                        // Try to cancel the task
                        match tm.cancel_task(task_id).await {
                            Ok(updated_task) => {
                                // Return the updated task
                                Ok(JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: request.id,
                                    result: Some(json!({
                                        "task": updated_task,
                                    })),
                                    error: None,
                                })
                            }
                            Err(e) => {
                                // Return error
                                Ok(JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: request.id,
                                    result: None,
                                    error: Some(JsonRpcError {
                                        code: 400,
                                        message: e.to_string(),
                                        data: None,
                                    }),
                                })
                            }
                        }
                    }
                    _ => Ok(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32601,
                            message: "Method not found".to_string(),
                            data: None,
                        }),
                    }),
                }
            }
        });
        
        // Run the transport with the handler
        runtime_handle.spawn(async move {
            if let Err(e) = transport.run(handler).await {
                error!("Failed to run transport: {}", e);
            }
        });
        
        info!("Server started");
        Ok(())
    }
    
    /// Run the server until shutdown signal
    pub fn run_until_shutdown(&self) -> Result<()> {
        self.start()?;
        
        self.runtime.block_on(async {
            // Wait for Ctrl+C
            if let Err(e) = ctrl_c().await {
                error!("Failed to listen for Ctrl+C: {}", e);
                return;
            }
            
            info!("Received shutdown signal, stopping server...");
            if let Err(e) = self.transport.stop().await {
                error!("Failed to stop transport: {}", e);
            }
            
            info!("Server stopped");
        });
        
        Ok(())
    }
}

