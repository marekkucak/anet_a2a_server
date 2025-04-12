use std::time::Duration;
use anyhow::{Context, Result};
use async_nats::{Client, ConnectOptions, Subject};
use chrono::Utc;
use futures::StreamExt; // Changed to futures::StreamExt
use log::{info, error};
use serde_json::{json, Value};
use tokio::time::timeout;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );
    
    info!("Starting A2A test client");
    
    // Connect to NATS
    info!("Connecting to NATS at nats://localhost:4222");
    let options = ConnectOptions::default();
    let nats = async_nats::connect_with_options("nats://localhost:4222", options).await?;
    
    // Test agent information endpoint
    test_get_agent_info(&nats).await?;
    
    // Test task creation and processing
    test_create_and_process_task(&nats).await?;
    
    // Test task streaming
    test_streaming_task(&nats).await?;
    
    info!("All tests completed successfully!");
    Ok(())
}

async fn test_get_agent_info(nats: &Client) -> Result<()> {
    info!("Testing agents/getInfo...");
    
    // Prepare request
    let request_id = Uuid::new_v4().to_string();
    let request = json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "agents/getInfo",
        "params": {}
    });
    
    // Send request
    let response: Value = send_request(nats, &request).await?;
    
    // Validate response
    if let Some(result) = response.get("result") {
        if let Some(agent) = result.get("agent") {
            info!("Agent info: {:?}", agent);
            return Ok(());
        }
    }
    
    error!("Invalid response: {:?}", response);
    Err(anyhow::anyhow!("Failed to get agent info"))
}

async fn test_create_and_process_task(nats: &Client) -> Result<()> {
    info!("Testing task creation and processing...");
    
    // Create a task
    let task_id = create_task(nats).await?;
    info!("Created task: {}", task_id);
    
    // Process the task
    process_task(nats, &task_id).await?;
    info!("Processed task: {}", task_id);
    
    // Get the task to verify it's completed
    let task = get_task(nats, &task_id).await?;
    info!("Task state: {:?}", task.get("state").and_then(|s| s.as_str()).unwrap_or("unknown"));
    
    Ok(())
}

async fn test_streaming_task(nats: &Client) -> Result<()> {
    info!("Testing streaming task processing...");
    
    // Create a task
    let task_id = create_task(nats).await?;
    info!("Created task for streaming: {}", task_id);
    
    // Process the task with streaming
    send_subscribe_task(nats, &task_id).await?;
    info!("Processed streaming task: {}", task_id);
    
    Ok(())
}

async fn create_task(nats: &Client) -> Result<String> {
    // Create a message
    let message_id = Uuid::new_v4();
    let now = Utc::now();
    let message = json!({
        "id": message_id.to_string(),
        "created_at": now.to_rfc3339(),
        "role": "user",
        "parts": [
            {
                "type": "text",
                "text": "Hello, can you help me with a task?"
            }
        ]
    });
    
    // Prepare request
    let request_id = Uuid::new_v4().to_string();
    let request = json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "tasks/create",
        "params": {
            "messages": [message]
        }
    });
    
    // Send request
    let response: Value = send_request(nats, &request).await?;
    
    // Extract task ID
    let task_id = response
        .get("result")
        .and_then(|r| r.get("task"))
        .and_then(|t| t.get("id"))
        .and_then(|id| id.as_str())
        .context("Failed to extract task ID")?;
    
    Ok(task_id.to_string())
}

async fn process_task(nats: &Client, task_id: &str) -> Result<Value> {
    // Prepare request
    let request_id = Uuid::new_v4().to_string();
    let request = json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "tasks/send",
        "params": {
            "taskId": task_id
        }
    });
    
    // Send request
    let response: Value = send_request(nats, &request).await?;
    
    // Extract task
    let task = response
        .get("result")
        .and_then(|r| r.get("task"))
        .context("Failed to extract task")?
        .clone();
    
    Ok(task)
}

async fn get_task(nats: &Client, task_id: &str) -> Result<Value> {
    // Prepare request
    let request_id = Uuid::new_v4().to_string();
    let request = json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "tasks/get",
        "params": {
            "taskId": task_id
        }
    });
    
    // Send request
    let response: Value = send_request(nats, &request).await?;
    
    // Extract task
    let task = response
        .get("result")
        .and_then(|r| r.get("task"))
        .context("Failed to extract task")?
        .clone();
    
    Ok(task)
}

async fn send_subscribe_task(nats: &Client, task_id: &str) -> Result<()> {
    // Prepare request
    let request_id = Uuid::new_v4().to_string();
    let request = json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "tasks/sendSubscribe",
        "params": {
            "taskId": task_id
        }
    });
    
    // Create a unique inbox for responses
    let inbox = nats.new_inbox();
    let mut subscription = nats.subscribe(Subject::from(inbox.clone())).await?;
    
    // Send request
    let payload = serde_json::to_vec(&request)?;
    nats.publish_with_reply(Subject::from("a2a.agent"), Subject::from(inbox), payload.into()).await?;
    
    // Collect streaming events
    info!("Waiting for streaming events...");
    let mut event_count = 0;
    
    while let Some(msg) = timeout(Duration::from_secs(10), subscription.next()).await? {
        let event: Value = serde_json::from_slice(&msg.payload)?;
        
        // Check if it's a JSON-RPC response (final message) or a streaming event
        if event.get("jsonrpc").is_some() {
            info!("Received final response: {:?}", event);
            break;
        } else {
            event_count += 1;
            let event_type = event.get("event").and_then(|e| e.as_str()).unwrap_or("unknown");
            info!("Received event #{}: {}", event_count, event_type);
        }
    }
    
    info!("Received {} streaming events", event_count);
    Ok(())
}

async fn send_request(nats: &Client, request: &Value) -> Result<Value> {
    // Create a unique inbox for the response
    let inbox = nats.new_inbox();
    let mut subscription = nats.subscribe(Subject::from(inbox.clone())).await?;
    
    // Send request
    let payload = serde_json::to_vec(request)?;
    nats.publish_with_reply(Subject::from("a2a.agent"), Subject::from(inbox), payload.into()).await?;
    
    // Wait for response
    let msg = timeout(Duration::from_secs(10), subscription.next())
        .await?
        .context("No response received")?;
    
    // Parse response
    let response: Value = serde_json::from_slice(&msg.payload)?;
    
    // Check for errors
    if let Some(error) = response.get("error") {
        error!("Error in response: {:?}", error);
        return Err(anyhow::anyhow!("Error in response: {:?}", error));
    }
    
    Ok(response)
}
