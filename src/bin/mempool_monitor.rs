use ethers::providers::{Provider, Ws, Middleware};
use ethers::types::{Address, Bytes};
use futures_util::StreamExt;
use std::sync::Arc;
use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    tracing::info!("🔍 Mempool Monitor Starting...");
    
    dotenv::dotenv().ok();
    let rpc_url = std::env::var("RPC_URL")
        .context("RPC_URL not set")?;
    let wallets_str = std::env::var("WALLETS_TO_TRACK")
        .context("WALLETS_TO_TRACK not set")?;
    
    let wallets: Vec<Address> = wallets_str
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    
    if wallets.is_empty() {
        tracing::error!("No valid wallet addresses found in WALLETS_TO_TRACK");
        anyhow::bail!("No valid wallet addresses");
    }
    
    tracing::info!("Tracking {} wallets", wallets.len());
    
    // Connect with retry logic
    let provider = connect_with_retry(&rpc_url, 5).await?;
    let provider = Arc::new(provider);
    
    tracing::info!("✅ Connected to RPC");
    
    // Subscribe to pending transactions
    let mut stream = provider
        .subscribe_pending_txs()
        .await
        .context("Failed to subscribe to mempool")?;
    
    tracing::info!("✅ Subscribed to mempool");
    tracing::info!("🎯 Monitoring pending transactions...");
    
    while let Some(tx_hash) = stream.next().await {
        // Get transaction details
        match provider.get_transaction(tx_hash).await {
            Ok(Some(tx)) => {
                // Check if transaction is from a tracked wallet
                if wallets.contains(&tx.from) {
                    tracing::info!("🔔 Detected pending tx from tracked wallet!");
                    tracing::info!("   From: {:?}", tx.from);
                    tracing::info!("   To: {:?}", tx.to);
                    tracing::info!("   Hash: {:?}", tx_hash);
                    tracing::info!("   Gas: {}", tx.gas);
                    tracing::info!("   Gas Price: {}", tx.gas_price.unwrap_or_default());
                    
                    // Decode transaction data (if it's a Polymarket trade)
                    if let Some(to) = tx.to {
                        if is_polymarket_contract(&to) {
                            tracing::info!("   ✅ This is a Polymarket trade!");
                            
                            // You can now execute a mirror trade BEFORE this tx is mined
                            // This gives you the same block execution
                            
                            // Parse trade details from tx.input
                            if let Some(trade_info) = parse_trade_data(&tx.input) {
                                tracing::info!("   Side: {:?}", trade_info.side);
                                tracing::info!("   Market: {}", trade_info.market_id);
                                tracing::info!("   Shares: {:.2}", trade_info.shares);
                                
                                // TODO: Execute mirror trade here
                                // execute_mirror_trade(trade_info).await;
                            }
                        }
                    }
                    
                    tracing::info!("---");
                }
            }
            Ok(None) => {
                // Transaction not found (might have been dropped)
            }
            Err(e) => {
                tracing::debug!("Error fetching transaction {}: {}", tx_hash, e);
            }
        }
    }
    
    Ok(())
}

async fn connect_with_retry(rpc_url: &str, max_retries: u32) -> Result<Provider<Ws>> {
    let mut attempts = 0;
    
    loop {
        attempts += 1;
        tracing::info!("Connecting to RPC (attempt {}/{})", attempts, max_retries);
        
        match Provider::<Ws>::connect(rpc_url).await {
            Ok(provider) => return Ok(provider),
            Err(e) => {
                if attempts >= max_retries {
                    return Err(anyhow::anyhow!("Failed to connect after {} attempts: {}", max_retries, e));
                }
                tracing::warn!("Connection failed: {}, retrying in {} seconds...", e, attempts * 2);
                tokio::time::sleep(tokio::time::Duration::from_secs((attempts * 2) as u64)).await;
            }
        }
    }
}

fn is_polymarket_contract(address: &Address) -> bool {
    // Polymarket CLOB contract addresses on Polygon
    let polymarket_contracts = [
        "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E", // Example CLOB
        "0xC5d563A36AE78145C45a50134d48A1215220f80a", // Example CLOB
    ];
    
    polymarket_contracts.iter().any(|&c| {
        c.parse::<Address>().ok() == Some(*address)
    })
}

#[derive(Debug)]
struct TradeInfo {
    side: String,
    market_id: String,
    shares: f64,
}

fn parse_trade_data(data: &Bytes) -> Option<TradeInfo> {
    // Parse transaction input data
    // This is simplified - actual parsing would decode ABI
    
    if data.len() < 36 {
        return None;
    }
    
    // Method selector (first 4 bytes)
    let selector = &data[0..4];
    
    // Common Polymarket function selectors:
    // 0x3d8b38f6 = placeBid
    // 0xc62e2971 = placeAsk
    // 0xa9059cbb = transfer (ERC20)
    
    // Simplified parsing
    Some(TradeInfo {
        side: if selector[0] % 2 == 0 { "BUY" } else { "SELL" }.to_string(),
        market_id: format!("0x{}", hex::encode(&data[4..36])),
        shares: 100.0, // Decode from data
    })
}
