#![allow(dead_code)]
#![allow(unused_imports)]

mod ai;
mod compute;
mod network;
mod storage;
mod api;
mod asic;
mod blockchain;
mod compute_pool;
mod consensus;
mod contract;
mod crypto;
mod economic;
mod integration;
mod marketplace;
mod merkle;
mod miner;
mod node;
mod p2p;
mod scheduler;
mod stark;
mod trace;
mod vm;
mod websocket;
mod workload;

use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    let api_port: u16 = args.get(1).and_then(|p| p.parse().ok()).unwrap_or(3000);

    tracing_subscriber::fmt().with_target(false).with_thread_ids(true).init();
    tracing::info!("Compute Chain v3.5.0");
    tracing::info!("Starting on port {}...", api_port);
    println!("");
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  COMPUTE CHAIN — v3.5.0                                    ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  📊 Dashboard:   http://localhost:{}/investor              ║", api_port);
    println!("║  🚀 Demo:        http://localhost:{}/demo                  ║", api_port);
    println!("║  🔗 Worker:      http://localhost:{}/worker                ║", api_port);
    println!("║  ❤️  Health:      http://localhost:{}/health                ║", api_port);
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("");
    api::server::start_server(api_port).await;

    Ok(())
}
