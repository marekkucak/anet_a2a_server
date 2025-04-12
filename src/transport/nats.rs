use std::{sync::Arc, time::Duration};
use anyhow::{Context, Result};
use async_nats::{Client, ConnectOptions, Subject};
use futures::StreamExt;
use log::{debug, error, info};
use serde_json::json;
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::types::{JsonRpcRequest, JsonRpcResponse, TaskStreamingEvent};
use crate::transport::Transport;

/// Transport implementation using NATS
pub struct NatsTransport {
    client: Mutex<Option<Client>>,
    url: String,
    subject: String,
    connect_timeout: Duration,
}

impl NatsTransport {
    /// Create a new NatsTransport
    pub fn new(url: &str, subject: &str, connect_timeout: Duration) -> Self {
        Self {
            client: Mutex::new(None),
            url: url.to_string(),
            subject: subject.to_string(),
            connect_timeout,
        }
    }

    /// Connect to NATS server
    async fn connect(&self) -> Result<Client> {
        let mut client_guard = self.client.lock().await;
        
        if let Some(client) = &*client_guard {
            return Ok(client.clone());
        }
        
        debug!("Connecting to NATS at {}", self.url);
        let options = ConnectOptions::default();
        
        let connect_fut = async_nats::connect_with_options(&self.url, options);
        let client = timeout(self.connect_timeout, connect_fut)
            .await
            .context("NATS connection timeout")?
            .context("Failed to connect to NATS")?;
        
        info!("Connected to NATS at {}", self.url);
        *client_guard = Some(client.clone());
        
        Ok(client)
    }
}

#[async_trait::async_trait]
impl Transport for NatsTransport {
    async fn start(&self) -> Result<()> {
        self.connect().await?;
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        let mut client_guard = self.client.lock().await;
        if let Some(_client) = client_guard.take() {
            // The async_nats::Client doesn't have a close method, so we just drop it
            info!("Closed NATS connection");
        }
        Ok(())
    }
    
    async fn run(&self, handler: Arc<dyn crate::transport::RequestHandler>) -> Result<()>
    {
        let client = self.connect().await?;
        let subject = Subject::from(self.subject.clone());
        
        info!("Subscribing to {}", self.subject);
        let mut subscription = client.subscribe(subject).await?;
        
        while let Some(msg) = subscription.next().await {
            let reply_subject = match msg.reply {
                Some(subject) => subject.to_string(),
                None => {
                    debug!("Received message without reply subject, skipping");
                    continue;
                }
            };
            
            let request: JsonRpcRequest = match serde_json::from_slice(&msg.payload) {
                Ok(req) => req,
                Err(e) => {
                    error!("Failed to parse JSON-RPC request: {}", e);
                    
                    // Send parse error response
                    let error_response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: "null".to_string(),
                        result: None,
                        error: Some(crate::types::JsonRpcError {
                            code: -32700,
                            message: "Parse error".to_string(),
                            data: Some(json!({ "error": e.to_string() })),
                        }),
                    };
                    
                    if let Err(e) = client
                        .publish(
                            Subject::from(reply_subject),
                            serde_json::to_vec(&error_response)?.into(),
                        )
                        .await
                    {
                        error!("Failed to send error response: {}", e);
                    }
                    
                    continue;
                }
            };
            
            // Special handling for streaming methods
            if request.method == "tasks/sendSubscribe" {
                let request_clone = request.clone();
                let client_clone = client.clone();
                let reply_subject_clone = reply_subject.clone();
                // We can't clone the handler directly, but we can clone the Arc
                let handler_clone = handler.clone();
                
                // Clone the request ID before moving the request
                let request_id = request_clone.id.clone();
                
                // Spawn a task to handle the streaming request
                tokio::spawn(async move {
                    // First create the task via the regular handler
                    let task_result = handler_clone.handle(request_clone).await;
                    
                    match task_result {
                        Ok(response) => {
                            if response.error.is_some() {
                                // If there was an error creating the task, just send the error response
                                if let Err(e) = client_clone
                                    .publish(
                                        Subject::from(reply_subject_clone),
                                        serde_json::to_vec(&response).unwrap_or_default().into(),
                                    )
                                    .await
                                {
                                    error!("Failed to send error response: {}", e);
                                }
                                return;
                            }
                            
                            // If task was created successfully, we can now handle the events stream
                            // Extract the task stream from the response
                            if let Some(result) = &response.result {
                                if let Some(events) = result.get("events").and_then(|e| e.as_array()) {
                                    for event in events {
                                        if let Ok(event) = serde_json::from_value::<TaskStreamingEvent>(event.clone()) {
                                            // Publish each event to the reply subject
                                            if let Err(e) = client_clone
                                                .publish(
                                                    Subject::from(reply_subject_clone.clone()),
                                                    serde_json::to_vec(&event).unwrap_or_default().into(),
                                                )
                                                .await
                                            {
                                                error!("Failed to publish streaming event: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                            
                            // Finally, send the JSON-RPC response (which the client will typically ignore)
                            if let Err(e) = client_clone
                                .publish(
                                    Subject::from(reply_subject_clone),
                                    serde_json::to_vec(&response).unwrap_or_default().into(),
                                )
                                .await
                            {
                                error!("Failed to send task creation response: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Error handling streaming request: {}", e);
                            // Send error response
                            let error_response = JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: request_id,
                                result: None,
                                error: Some(crate::types::JsonRpcError {
                                    code: -32603,
                                    message: "Internal error".to_string(),
                                    data: Some(json!({ "error": e.to_string() })),
                                }),
                            };
                            
                            if let Err(e) = client_clone
                                .publish(
                                    Subject::from(reply_subject_clone),
                                    serde_json::to_vec(&error_response).unwrap_or_default().into(),
                                )
                                .await
                            {
                                error!("Failed to send error response: {}", e);
                            }
                        }
                    }
                });
            } else {
                // Handle regular (non-streaming) requests
                let request_clone = request.clone();
                let client_clone = client.clone();
                let reply_subject_clone = reply_subject.clone();
                let handler_clone = handler.clone(); // Clone the Arc<RequestHandler> before moving
                
                // Clone the request ID before moving the request
                let request_id = request_clone.id.clone();
                
                tokio::spawn(async move {
                    match handler_clone.handle(request_clone).await {
                        Ok(response) => {
                            if let Err(e) = client_clone
                                .publish(
                                    Subject::from(reply_subject_clone),
                                    serde_json::to_vec(&response).unwrap_or_default().into(),
                                )
                                .await
                            {
                                error!("Failed to send response: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Error handling request: {}", e);
                            // Send error response
                            let error_response = JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: request_id,
                                result: None,
                                error: Some(crate::types::JsonRpcError {
                                    code: -32603,
                                    message: "Internal error".to_string(),
                                    data: Some(json!({ "error": e.to_string() })),
                                }),
                            };
                            
                            if let Err(e) = client_clone
                                .publish(
                                    Subject::from(reply_subject_clone),
                                    serde_json::to_vec(&error_response).unwrap_or_default().into(),
                                )
                                .await
                            {
                                error!("Failed to send error response: {}", e);
                            }
                        }
                    }
                });
            }
        }
        
        Ok(())
    }
    
    async fn publish_event(&self, reply_subject: &str, event: TaskStreamingEvent) -> Result<()> {
        let client = self.connect().await?;
        
        client
            .publish(
                Subject::from(reply_subject),
                serde_json::to_vec(&event)?.into(),
            )
            .await
            .context("Failed to publish event")?;
            
        Ok(())
    }
}

/// Factory for creating NatsTransport instances
pub struct NatsTransportFactory {
    url: String,
    subject: String,
    connect_timeout: Duration,
}

impl NatsTransportFactory {
    /// Create a new NatsTransportFactory
    pub fn new(url: &str, subject: &str, connect_timeout: Duration) -> Self {
        Self {
            url: url.to_string(),
            subject: subject.to_string(),
            connect_timeout,
        }
    }
}

impl crate::transport::TransportFactory for NatsTransportFactory {
    fn create(&self) -> Result<Arc<dyn Transport>> {
        Ok(Arc::new(NatsTransport::new(
            &self.url,
            &self.subject,
            self.connect_timeout,
        )))
    }
}
