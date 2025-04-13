// src/server.rs

use std::sync::Arc;
use anyhow::Result;
use log::{error, info};
use serde_json::json;
use tokio::runtime::Runtime;
use tokio::signal;
use tokio::sync::oneshot;
use uuid::Uuid;
use futures::StreamExt;

use crate::task_manager_trait::TaskManagerTrait;
use crate::transport::{Transport, create_handler};
use crate::types::{JsonRpcRequest, create_response, create_error_response};

/// Server for handling A2A requests
pub struct Server {
    task_manager: Arc<dyn TaskManagerTrait>,
    transport: Arc<dyn Transport>,
    runtime: Runtime,
}

impl Server {
    /// Create a new Server instance
    pub(crate) fn new(
        task_manager: Arc<dyn TaskManagerTrait>,
        transport: Arc<dyn Transport>,
        runtime: Runtime,
    ) -> Self {
        Self {
            task_manager,
            transport,
            runtime,
        }
    }
    
    /// Run the server until shutdown signal is received
    pub fn run_until_shutdown(self) -> Result<()> {
        let (_tx, rx) = oneshot::channel::<()>();
        
        // Start the server in the runtime
        self.runtime.block_on(async {
            // Start the transport
            self.transport.start().await?;
            
            // Create and register request handler
            let task_manager = self.task_manager.clone();
            let handler = create_handler(move |request: JsonRpcRequest| {
                let task_manager = task_manager.clone();
                async move {
                    match request.method.as_str() {
                        "agents/getInfo" => {
                            let agent = task_manager.agent();
                            let response = create_response(
                                request.id,
                                json!({
                                    "agent": agent,
                                }),
                            );
                            Ok(response)
                        }
                        "tasks/create" => {
                            // Extract messages from params
                            match request.params.get("messages") {
                                Some(messages_value) => {
                                    // Parse messages
                                    let messages = match serde_json::from_value(messages_value.clone()) {
                                        Ok(m) => m,
                                        Err(e) => {
                                            return Ok(create_error_response(
                                                request.id,
                                                -32602,
                                                format!("Invalid messages parameter: {}", e),
                                                None,
                                            ));
                                        }
                                    };
                                    
                                    // Extract push notification config if present
                                    let push_notification_config = request
                                        .params
                                        .get("pushNotificationConfig")
                                        .and_then(|v| serde_json::from_value(v.clone()).ok());
                                    
                                    // Create task
                                    match task_manager.create_task(messages, push_notification_config).await {
                                        Ok(task) => {
                                            let response = create_response(
                                                request.id,
                                                json!({
                                                    "task": task,
                                                }),
                                            );
                                            Ok(response)
                                        }
                                        Err(e) => {
                                            Ok(create_error_response(
                                                request.id,
                                                -32603,
                                                format!("Failed to create task: {}", e),
                                                None,
                                            ))
                                        }
                                    }
                                }
                                None => {
                                    Ok(create_error_response(
                                        request.id,
                                        -32602,
                                        "Missing messages parameter".to_string(),
                                        None,
                                    ))
                                }
                            }
                        }
                        "tasks/get" => {
                            // Extract task ID from params
                            match request.params.get("taskId") {
                                Some(task_id_value) => {
                                    // Parse task ID
                                    let task_id_str = match task_id_value.as_str() {
                                        Some(s) => s,
                                        None => {
                                            return Ok(create_error_response(
                                                request.id,
                                                -32602,
                                                "Invalid taskId parameter, must be a string".to_string(),
                                                None,
                                            ));
                                        }
                                    };
                                    
                                    // Parse UUID
                                    let task_id = match Uuid::parse_str(task_id_str) {
                                        Ok(id) => id,
                                        Err(e) => {
                                            return Ok(create_error_response(
                                                request.id,
                                                -32602,
                                                format!("Invalid UUID format: {}", e),
                                                None,
                                            ));
                                        }
                                    };
                                    
                                    // Get task
                                    match task_manager.get_task(task_id).await {
                                        Ok(Some(task)) => {
                                            let response = create_response(
                                                request.id,
                                                json!({
                                                    "task": task,
                                                }),
                                            );
                                            Ok(response)
                                        }
                                        Ok(None) => {
                                            Ok(create_error_response(
                                                request.id,
                                                -32001,
                                                format!("Task not found: {}", task_id),
                                                None,
                                            ))
                                        }
                                        Err(e) => {
                                            Ok(create_error_response(
                                                request.id,
                                                -32603,
                                                format!("Failed to get task: {}", e),
                                                None,
                                            ))
                                        }
                                    }
                                }
                                None => {
                                    Ok(create_error_response(
                                        request.id,
                                        -32602,
                                        "Missing taskId parameter".to_string(),
                                        None,
                                    ))
                                }
                            }
                        }
                        "tasks/send" => {
                            // Extract task ID from params
                            match request.params.get("taskId") {
                                Some(task_id_value) => {
                                    // Parse task ID
                                    let task_id_str = match task_id_value.as_str() {
                                        Some(s) => s,
                                        None => {
                                            return Ok(create_error_response(
                                                request.id,
                                                -32602,
                                                "Invalid taskId parameter, must be a string".to_string(),
                                                None,
                                            ));
                                        }
                                    };
                                    
                                    // Parse UUID
                                    let task_id = match Uuid::parse_str(task_id_str) {
                                        Ok(id) => id,
                                        Err(e) => {
                                            return Ok(create_error_response(
                                                request.id,
                                                -32602,
                                                format!("Invalid UUID format: {}", e),
                                                None,
                                            ));
                                        }
                                    };
                                    
                                    // Process task
                                    match task_manager.process_task(task_id).await {
                                        Ok(task) => {
                                            let response = create_response(
                                                request.id,
                                                json!({
                                                    "task": task,
                                                }),
                                            );
                                            Ok(response)
                                        }
                                        Err(e) => {
                                            Ok(create_error_response(
                                                request.id,
                                                -32603,
                                                format!("Failed to process task: {}", e),
                                                None,
                                            ))
                                        }
                                    }
                                }
                                None => {
                                    Ok(create_error_response(
                                        request.id,
                                        -32602,
                                        "Missing taskId parameter".to_string(),
                                        None,
                                    ))
                                }
                            }
                        }
                        "tasks/sendSubscribe" => {
                            // Extract task ID from params
                            match request.params.get("taskId") {
                                Some(task_id_value) => {
                                    // Parse task ID
                                    let task_id_str = match task_id_value.as_str() {
                                        Some(s) => s,
                                        None => {
                                            return Ok(create_error_response(
                                                request.id,
                                                -32602,
                                                "Invalid taskId parameter, must be a string".to_string(),
                                                None,
                                            ));
                                        }
                                    };
                                    
                                    // Parse UUID
                                    let task_id = match Uuid::parse_str(task_id_str) {
                                        Ok(id) => id,
                                        Err(e) => {
                                            return Ok(create_error_response(
                                                request.id,
                                                -32602,
                                                format!("Invalid UUID format: {}", e),
                                                None,
                                            ));
                                        }
                                    };
                                    
                                    // Process task with streaming
                                    match task_manager.process_task_streaming(task_id).await {
                                        Ok(stream) => {
                                            // Collect all events to return in the response
                                            // Use StreamExt::collect instead of Iterator::collect
                                            let events = stream.collect::<Vec<_>>().await;
                                            
                                            let response = create_response(
                                                request.id,
                                                json!({
                                                    "events": events,
                                                }),
                                            );
                                            Ok(response)
                                        }
                                        Err(e) => {
                                            Ok(create_error_response(
                                                request.id,
                                                -32603,
                                                format!("Failed to process streaming task: {}", e),
                                                None,
                                            ))
                                        }
                                    }
                                }
                                None => {
                                    Ok(create_error_response(
                                        request.id,
                                        -32602,
                                        "Missing taskId parameter".to_string(),
                                        None,
                                    ))
                                }
                            }
                        }
                        "tasks/cancel" => {
                            // Extract task ID from params
                            match request.params.get("taskId") {
                                Some(task_id_value) => {
                                    // Parse task ID
                                    let task_id_str = match task_id_value.as_str() {
                                        Some(s) => s,
                                        None => {
                                            return Ok(create_error_response(
                                                request.id,
                                                -32602,
                                                "Invalid taskId parameter, must be a string".to_string(),
                                                None,
                                            ));
                                        }
                                    };
                                    
                                    // Parse UUID
                                    let task_id = match Uuid::parse_str(task_id_str) {
                                        Ok(id) => id,
                                        Err(e) => {
                                            return Ok(create_error_response(
                                                request.id,
                                                -32602,
                                                format!("Invalid UUID format: {}", e),
                                                None,
                                            ));
                                        }
                                    };
                                    
                                    // Cancel task
                                    match task_manager.cancel_task(task_id).await {
                                        Ok(task) => {
                                            let response = create_response(
                                                request.id,
                                                json!({
                                                    "task": task,
                                                }),
                                            );
                                            Ok(response)
                                        }
                                        Err(e) => {
                                            Ok(create_error_response(
                                                request.id,
                                                -32603,
                                                format!("Failed to cancel task: {}", e),
                                                None,
                                            ))
                                        }
                                    }
                                }
                                None => {
                                    Ok(create_error_response(
                                        request.id,
                                        -32602,
                                        "Missing taskId parameter".to_string(),
                                        None,
                                    ))
                                }
                            }
                        }
                        _ => {
                            Ok(create_error_response(
                                request.id,
                                -32601,
                                format!("Method not found: {}", request.method),
                                None,
                            ))
                        }
                    }
                }
            });
            
            // Clone transport before moving it into the spawned task
            let transport_clone = self.transport.clone();
            
            // Run the transport with the handler
            let transport_handle = tokio::spawn(async move {
                if let Err(e) = transport_clone.run(handler).await {
                    error!("Transport error: {}", e);
                }
            });
            
            // Wait for shutdown signal
            tokio::select! {
                _ = signal::ctrl_c() => {
                    info!("Received Ctrl+C, shutting down...");
                }
                _ = rx => {
                    info!("Received shutdown signal, shutting down...");
                }
            }
            
            // Abort the transport handler
            transport_handle.abort();
            
            // Stop the transport
            if let Err(e) = self.transport.stop().await {
                error!("Error stopping transport: {}", e);
            }
            
            info!("Server shutdown complete");
            
            Ok::<_, anyhow::Error>(())
        })?;
        
        Ok(())
    }
    
    /// Request server shutdown
    pub fn shutdown(&self) -> Result<()> {
        // Currently not implemented - would send a signal to the shutdown channel
        Ok(())
    }
}
