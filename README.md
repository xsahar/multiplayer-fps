# Maze Wars

A multiplayer, raycast-rendered (Wolfenstein-style) FPS set in procedurally generated mazes. Written in Rust with [macroquad](https://github.com/not-fl3/macroquad) for rendering and a custom UDP protocol for networking.

## Features

- **Procedural mazes** — generated per-round with a randomized Prim's algorithm, then "braided" to knock out dead ends and add loops/shortcuts (`src/common/maze.rs`).
- **Three difficulty levels** — `Easy` (23x15), `Normal` (39x27), `Hard` (55x37), plus map voting and a `Random` choice.
- **Lobby → Match flow** — players join a lobby, vote on a map, and the match starts once the lobby timer runs out.
- **Raycast renderer** — classic grid-based raycasting for walls, with sprite-based rendering for other players, fog, and a neon HUD.
- **Live scoreboard** — kills, deaths, and score are tracked per player and broadcast every snapshot; a standings screen shows at match end.
- **In-game chat and hit markers**, spawn protection, and ping/latency tracking.

## Project layout

```
src/
├── bin/
│   ├── client.rs      # client entry point, UI/menu, input handling
│   └── server.rs      # server entry point (binds and runs GameServer)
├── client/
│   ├── network.rs     # UDP client: join, input, vote, chat, keepalive
│   └── renderer.rs     # raycasting renderer, HUD, sprites
├── server/
│   └── game_server.rs # authoritative game loop, lobby/match state, hit detection
└── common/
    ├── protocol.rs    # wire types shared by client & server (bincode-serialized)
    ├── maze.rs        # maze generation
    └── player.rs      # server-side player state
```

## Networking

The client and server talk over UDP using `bincode`-encoded messages defined in `src/common/protocol.rs`:

- `ClientToServer`: `Join`, `Input`, `Vote`, `Ping`, `ReturnToLobby`, `ChatMessage`
- `ServerToClient`: `JoinOk`, `JoinRejected`, `Snapshot`, `LevelChanged`, `LobbyState`, `MatchOver`, `HitConfirm`, `Pong`, `ServerFull`, `ChatMessage`

The server runs an authoritative simulation at `SERVER_TICK_HZ` (30 Hz) and broadcasts world snapshots at `SNAPSHOT_HZ` (20 Hz). Default port is `45000`, with up to `MAX_PLAYERS` (64) concurrent players.

## Building & running

> **Note:** this checkout is missing a `Cargo.toml` (only `Cargo.lock` is present, listing the crate as `maze-wars` with dependencies on `anyhow`, `bincode`, `crossbeam-channel`, `macroquad`, `rand`, `serde`, and `thiserror`). Add a `Cargo.toml` declaring the two binaries (`src/bin/server.rs`, `src/bin/client.rs`) before these commands will work.

Run the server:

```sh
cargo run --bin server
# or specify a bind address:
cargo run --bin server -- 0.0.0.0:45000
# or via env var:
MAZE_WARS_BIND=0.0.0.0:45000 cargo run --bin server
```

Run the client:

```sh
cargo run --bin client
```

## Requirements

- Rust (stable toolchain)
- macroquad's platform dependencies (see [macroquad's README](https://github.com/not-fl3/macroquad) for OS-specific requirements, e.g. X11/Wayland dev libs on Linux)
