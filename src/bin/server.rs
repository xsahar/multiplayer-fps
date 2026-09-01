use maze_wars::server::game_server::GameServer;
use std::env;

fn main() -> anyhow::Result<()> {
    // Priority: CLI arg > MAZE_WARS_BIND env > default 0.0.0.0:45000
    let bind = env::args()
        .nth(1)
        .or_else(|| env::var("MAZE_WARS_BIND").ok())
        .unwrap_or_else(|| format!("0.0.0.0:{}", maze_wars::common::protocol::DEFAULT_PORT));

    println!("[Server] Binding to {}", bind);
    let mut server = GameServer::bind(&bind)?;
    server.run();
    Ok(())
}
