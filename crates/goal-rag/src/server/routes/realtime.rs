//! WebSocket real-time API endpoint
//!
//! Provides WebSocket connections for subscribing to PostgreSQL database changes.
//! Frontend connects and subscribes to specific tables/events.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::server::state::AppState;

/// WebSocket subscription request
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum WsRequest {
    /// Subscribe to a table's changes
    #[serde(rename = "subscribe")]
    Subscribe {
        table: String,
        #[serde(default)]
        event: Option<String>, // INSERT, UPDATE, DELETE, or * for all
        #[serde(default)]
        filter: Option<String>, // e.g., "organization_id=eq.abc123"
    },
    /// Unsubscribe from a table
    #[serde(rename = "unsubscribe")]
    Unsubscribe { table: String },
    /// Ping to keep connection alive
    #[serde(rename = "ping")]
    Ping,
}

/// WebSocket response/event
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum WsResponse {
    /// Subscription confirmed
    #[serde(rename = "subscribed")]
    Subscribed { table: String, subscription_id: String },
    /// Unsubscribed
    #[serde(rename = "unsubscribed")]
    Unsubscribed { table: String },
    /// Database change event
    #[serde(rename = "change")]
    Change {
        table: String,
        event: String,
        new_record: Option<serde_json::Value>,
        old_record: Option<serde_json::Value>,
    },
    /// Pong response
    #[serde(rename = "pong")]
    Pong,
    /// Error
    #[serde(rename = "error")]
    Error { message: String },
    /// Connection established
    #[serde(rename = "connected")]
    Connected { message: String },
}

/// Allowed tables for subscription (security whitelist)
const ALLOWED_TABLES: &[&str] = &[
    "tasks",
    "goals",
    "users",
    "organizations",
    "documents",
    "conversations",
    "chat_messages",
    "task_comments",
    "task_attachments",
    "messages",
    "categories",
    "groups",
    "special_events",
];

/// Validate table name
fn validate_table(table: &str) -> Result<(), String> {
    if !ALLOWED_TABLES.contains(&table) {
        return Err(format!(
            "Table '{}' is not allowed. Allowed tables: {}",
            table,
            ALLOWED_TABLES.join(", ")
        ));
    }
    Ok(())
}

/// WebSocket handler - upgrades HTTP to WebSocket
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(_state): State<AppState>,
) -> Response {
    ws.on_upgrade(handle_socket)
}

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();

    // Track subscriptions for this connection
    let subscriptions: Arc<RwLock<HashSet<String>>> = Arc::new(RwLock::new(HashSet::new()));

    // Send connected message
    let connected = WsResponse::Connected {
        message: "WebSocket connection established. Subscribe to tables to receive changes.".to_string(),
    };
    if let Ok(msg) = serde_json::to_string(&connected) {
        let _ = sender.send(Message::Text(msg.into())).await;
    }

    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let response = handle_message(&text, &subscriptions).await;
                if let Ok(json) = serde_json::to_string(&response) {
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
            Ok(Message::Ping(data)) => {
                if sender.send(Message::Pong(data)).await.is_err() {
                    break;
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }

    tracing::info!("WebSocket connection closed");
}

/// Handle incoming WebSocket message
async fn handle_message(
    text: &str,
    subscriptions: &Arc<RwLock<HashSet<String>>>,
) -> WsResponse {
    // Parse request
    let request: WsRequest = match serde_json::from_str(text) {
        Ok(req) => req,
        Err(e) => {
            return WsResponse::Error {
                message: format!("Invalid JSON: {}", e),
            };
        }
    };

    match request {
        WsRequest::Subscribe { table, event, filter } => {
            // Validate table
            if let Err(e) = validate_table(&table) {
                return WsResponse::Error { message: e };
            }

            // Create subscription ID
            let event_str = event.as_deref().unwrap_or("*");
            let filter_str = filter.as_deref().unwrap_or("");
            let sub_id = format!("{}:{}:{}", table, event_str, filter_str);

            // Add to subscriptions
            {
                let mut subs = subscriptions.write().await;
                subs.insert(sub_id.clone());
            }

            tracing::info!(
                "Client subscribed to table '{}' (event: {}, filter: {:?})",
                table, event_str, filter
            );

            WsResponse::Subscribed {
                table,
                subscription_id: sub_id,
            }
        }

        WsRequest::Unsubscribe { table } => {
            // Remove all subscriptions for this table
            {
                let mut subs = subscriptions.write().await;
                subs.retain(|s| !s.starts_with(&format!("{}:", table)));
            }

            tracing::info!("Client unsubscribed from table '{}'", table);

            WsResponse::Unsubscribed { table }
        }

        WsRequest::Ping => WsResponse::Pong,
    }
}

/// Broadcast a change event to all connected clients
/// This would be called by a PostgreSQL NOTIFY listener
#[allow(dead_code)]
pub fn create_change_event(
    table: &str,
    event: &str,
    new_record: Option<serde_json::Value>,
    old_record: Option<serde_json::Value>,
) -> WsResponse {
    WsResponse::Change {
        table: table.to_string(),
        event: event.to_string(),
        new_record,
        old_record,
    }
}
