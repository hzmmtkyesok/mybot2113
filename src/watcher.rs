use crate::types::{Trade, TradeSide};
use anyhow::{Context, Result};
use async_channel::{Sender, Receiver, bounded};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub struct WalletWatcher {
    ws_url: String,
    wallets: Vec<String>,
}

impl WalletWatcher {
    pub fn new(ws_url: String, wallets: Vec<String>) -> Self {
        Self { ws_url, wallets }
    }
    
    pub async fn start(&self) -> Result<Receiver<Trade>> {
        let (tx, rx) = bounded(1000);
        
        for wallet in &self.wallets {
            let wallet_clone = wallet.clone();
            let ws_url = self.ws_url.clone();
            let tx_clone = tx.clone();
            
            tokio::spawn(async move {
                if let Err(e) = watch_wallet(ws_url, wallet_clone, tx_clone).await {
                    tracing::error!("Wallet watcher error: {}", e);
                }
            });
        }
        
        Ok(rx)
    }
}

async fn watch_wallet(ws_url: String, wallet: String, tx: Sender<Trade>) -> Result<()> {
    let mut retry_count = 0;
    let max_retries = 10;
    let base_delay = 5;
    
    loop {
        tracing::info!("Attempting WebSocket connection for wallet {}...", &wallet[..10.min(wallet.len())]);
        
        match connect_and_watch(&ws_url, &wallet, &tx).await {
            Ok(_) => {
                tracing::info!("WebSocket connection closed normally for {}", &wallet[..10.min(wallet.len())]);
                retry_count = 0; // Reset on successful connection
            }
            Err(e) => {
                retry_count += 1;
                let delay = base_delay * retry_count.min(6); // Max 30 seconds delay
                
                tracing::error!(
                    "WebSocket error for {} (attempt {}/{}): {}",
                    &wallet[..10.min(wallet.len())],
                    retry_count,
                    max_retries,
                    e
                );
                
                if retry_count >= max_retries {
                    tracing::error!("Max retries reached for wallet {}, will continue trying with longer delays", &wallet[..10.min(wallet.len())]);
                    retry_count = max_retries / 2; // Reset to half to keep trying
                }
                
                tracing::info!("Reconnecting in {} seconds...", delay);
                tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
            }
        }
    }
}

async fn connect_and_watch(ws_url: &str, wallet: &str, tx: &Sender<Trade>) -> Result<()> {
    // Parse and validate WebSocket URL
    let url = url::Url::parse(ws_url)
        .context("Invalid WebSocket URL")?;
    
    tracing::debug!("Connecting to WebSocket: {}", url);
    
    // Connect with timeout
    let connect_future = connect_async(url.as_str());
    let (ws_stream, response) = tokio::time::timeout(
        tokio::time::Duration::from_secs(30),
        connect_future
    )
    .await
    .context("WebSocket connection timeout")?
    .context("Failed to connect to WebSocket")?;
    
    tracing::info!("WebSocket connected, HTTP status: {}", response.status());
    
    let (write, mut read) = ws_stream.split();
    let write = Arc::new(Mutex::new(write));
    let is_connected = Arc::new(AtomicBool::new(true));
    
    // Subscribe to wallet trades
    let subscribe_msg = json!({
        "type": "subscribe",
        "channel": "trades",
        "wallet": wallet,
    });
    
    {
        let mut write_guard = write.lock().await;
        write_guard.send(Message::Text(subscribe_msg.to_string()))
            .await
            .context("Failed to send subscribe message")?;
    }
    
    tracing::info!("Subscribed to trades for wallet: {}", &wallet[..10.min(wallet.len())]);
    
    // Keep connection alive with ping
    let write_clone = Arc::clone(&write);
    let is_connected_clone = Arc::clone(&is_connected);
    let ping_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            
            if !is_connected_clone.load(Ordering::Relaxed) {
                tracing::debug!("Ping task stopping - connection closed");
                break;
            }
            
            let mut write_guard = write_clone.lock().await;
            if write_guard.send(Message::Ping(vec![])).await.is_err() {
                tracing::warn!("Failed to send ping, connection may be lost");
                break;
            }
            tracing::debug!("Ping sent");
        }
    });
    
    // Process incoming messages
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                tracing::debug!("Received message: {}", &text[..100.min(text.len())]);
                
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                    // Handle different event types
                    if let Some(event_type) = event["type"].as_str() {
                        match event_type {
                            "trade" => {
                                if let Some(trade) = parse_trade_event(&event, wallet) {
                                    if let Err(e) = tx.send(trade).await {
                                        tracing::error!("Failed to send trade to channel: {}", e);
                                        break;
                                    }
                                }
                            }
                            "subscribed" => {
                                tracing::info!("Successfully subscribed to channel");
                            }
                            "error" => {
                                let error_msg = event["message"].as_str().unwrap_or("Unknown error");
                                tracing::error!("WebSocket server error: {}", error_msg);
                            }
                            "heartbeat" | "pong" => {
                                tracing::debug!("Heartbeat received");
                            }
                            _ => {
                                tracing::debug!("Unknown event type: {}", event_type);
                            }
                        }
                    }
                }
            }
            Ok(Message::Pong(_)) => {
                tracing::debug!("Pong received - connection alive");
            }
            Ok(Message::Ping(data)) => {
                // Respond to server pings
                let mut write_guard = write.lock().await;
                if let Err(e) = write_guard.send(Message::Pong(data)).await {
                    tracing::warn!("Failed to send pong: {}", e);
                }
            }
            Ok(Message::Close(frame)) => {
                if let Some(cf) = frame {
                    tracing::warn!("WebSocket closed by server: {} - {}", cf.code, cf.reason);
                } else {
                    tracing::warn!("WebSocket closed by server (no close frame)");
                }
                break;
            }
            Ok(Message::Binary(data)) => {
                tracing::debug!("Received binary data: {} bytes", data.len());
            }
            Ok(Message::Frame(_)) => {
                // Raw frame, usually not needed
            }
            Err(e) => {
                tracing::error!("WebSocket error: {}", e);
                break;
            }
        }
    }
    
    // Clean up
    is_connected.store(false, Ordering::Relaxed);
    ping_task.abort();
    
    Ok(())
}

fn parse_trade_event(event: &serde_json::Value, wallet: &str) -> Option<Trade> {
    let event_type = event["type"].as_str()?;
    
    if event_type != "trade" {
        return None;
    }
    
    let data = &event["data"];
    
    // Handle both nested and flat data structures
    let get_field = |field: &str| -> Option<&serde_json::Value> {
        if data[field].is_null() {
            event.get(field)
        } else {
            Some(&data[field])
        }
    };
    
    let event_id = get_field("event_id")?.as_str()?.to_string();
    let market_id = get_field("market_id")?.as_str()?.to_string();
    let side_str = get_field("side")?.as_str()?;
    let shares = get_field("shares")?.as_f64()?;
    let price = get_field("price")?.as_f64()?;
    let timestamp = get_field("timestamp")?.as_i64()?;
    let tx_hash = get_field("tx_hash").and_then(|v| v.as_str()).map(|s| s.to_string());
    
    let side = match side_str.to_uppercase().as_str() {
        "BUY" => TradeSide::BUY,
        "SELL" => TradeSide::SELL,
        _ => return None,
    };
    
    Some(Trade {
        wallet: wallet.to_string(),
        event_id,
        market_id,
        side,
        shares,
        price,
        timestamp,
        tx_hash,
    })
}
