# Build Your Own Multiplayer FPS Checklist

Use this file as a study and implementation path. Do not begin with a complete online shooter. Build a small version, test it, and add one system at a time.

## Current Progress

Status: `[x]` complete, `[~]` started but incomplete, `[ ]` not started.

- [x] Cargo project with library, server binary, and client binary
- [x] Shared `Player` and `Maze` types
- [x] Fixed offline maze with walls, floor, safe tile lookup, and spawn lookup
- [x] Basic movement and wall/boundary checks
- [x] Binary serialization with Serde and bincode
- [x] UDP server receives and decodes one client join packet
- [x] UDP client sends one join packet
- [x] Server sends and client decodes `JoinAccepted`
- [x] Continuous server loop and multiple connected players
- [x] Snapshot broadcast to connected clients after move and after join
- [x] Minimal client-side snapshot processing loop for state updates
- [x] Continuous client message loop while the app is running
- [x] Ping and pong protocol messages for connection health checks
- [x] Basic lobby state and phase tracking on the server
- [x] Round reset and match-start flow
- [x] Minimal graphical client window with maze and player drawing
- [x] First raycast pass with wall distance and vertical wall strips
- [~] Minimap and FPS display

### Next Session

Add player camera rotation, then finish the minimap and FPS overlay on top of
the raycast view.

## How to Use This Guide

For every milestone:

1. Learn the listed concepts.
2. Implement only that milestone.
3. Run the listed test.
4. Do not continue until the test works.
5. Commit your own work if you use Git.

## Milestone 0: Prepare the Project

Learn:

- Rust toolchains and Cargo
- Modules and crates
- Structs, enums, traits, `Option`, and `Result`
- Ownership, borrowing, and collections

Create:

- A Cargo project
- A library module for shared types
- A server binary
- A client binary
- A basic error-handling strategy

Test:

```sh
cargo check
cargo run --bin server
cargo run --bin client
```

Success condition: both binaries start without networking or graphics errors.

## Milestone 1: Make an Offline Maze

Learn:

- `Vec` and two-dimensional indexing
- Random number generation
- Grid coordinates
- Procedural generation

Implement:

- `Maze { width, height, tiles }`
- `get` and `set`
- Wall and floor values
- A simple maze algorithm
- A random floor spawn point

Test:

- Print the maze using `#` for walls and `.` for floor.
- Confirm the border is always a wall.
- Confirm the spawn point is always on a floor tile.

## Milestone 2: Add a Local Player

Learn:

- Vectors and angles
- Radians and trigonometry
- Frame time (`dt`)
- Keyboard and mouse input

Implement:

- Player position `(x, y)`
- Player yaw
- Forward and strafe movement
- Mouse turning
- Movement speed

Test:

- Walk in every direction.
- Rotate while moving.
- Confirm movement speed does not depend heavily on frame rate.

## Milestone 3: Add Collision

Learn:

- Circle-versus-rectangle collision
- Axis-separated movement
- Sliding along walls
- Boundary checking

Implement:

```text
candidate = current_position + requested_movement
if candidate is free:
    apply candidate
else:
    try X movement separately
    try Y movement separately
```

Test:

- Walk into every wall.
- Confirm the player cannot leave the map.
- Confirm the player cannot pass through corners.

## Milestone 4: Add Raycast Rendering

Learn:

- Camera projection
- Ray/line intersection
- Depth and perspective
- Drawing vertical strips

Implement:

- One ray per screen column
- Wall distance calculation
- Wall height calculation
- Ceiling and floor colors
- Crosshair

Test:

- Walls look taller when close.
- Turning the player turns the view.
- The maze is navigable and not visually blank.

## Milestone 5: Define the Protocol

Learn:

- Serialization
- Versioning shared data
- Enums for message types
- Network-safe data design

Create shared types such as:

```rust
struct JoinRequest {
    username: String,
}

struct InputState {
    forward: f32,
    strafe: f32,
    yaw_delta: f32,
    fire: bool,
}

struct PlayerNet {
    id: u32,
    x: f32,
    y: f32,
    yaw: f32,
    hp: i32,
}
```

Create messages for:

- Join request and response
- Input
- Snapshot
- Ping and pong
- Error or rejection

Test:

- Serialize each message.
- Deserialize it again.
- Confirm the original and decoded values match.

## Milestone 6: Build the Server

Learn:

- UDP sockets
- `SocketAddr`
- Nonblocking I/O
- Server loops
- Hash maps keyed by player ID

Implement:

- Bind a UDP socket.
- Receive bytes.
- Deserialize messages.
- Track address-to-player-ID mappings.
- Accept valid joins.
- Store players on the server.
- Send a join response.

Test:

- Start the server.
- Join from one client.
- See the server log the player.
- Reject invalid usernames.
- Reject duplicate usernames.

## Milestone 7: Send Input and Snapshots

Learn:

- Client/server authority
- Tick rates
- Snapshot frequency
- Timeouts and keepalives

Implement:

```text
Client: read input -> send InputState
Server: receive input -> simulate player
Server: send Snapshot
Client: receive snapshot -> update display
```

Test:

- Move a player while the server is running.
- Stop sending keepalives and confirm the server eventually removes the player.
- Confirm the client does not permanently freeze when a packet is lost.

## Milestone 8: Support Two Clients

Learn:

- Replicated entities
- Multiple sockets and addresses
- Remote-player rendering
- Interpolation

Implement:

- A player collection on the server.
- A unique ID for every player.
- A snapshot containing all players.
- Local-player identification.
- Remote-player billboards.

Test:

- Start the server.
- Start two clients with different usernames.
- Move each client.
- Confirm both clients see both players.
- Confirm one client disconnecting does not crash the server.

## Milestone 9: Add Combat

Learn:

- Raycasting against players
- Point-to-ray distance
- Cooldowns
- Health and respawning
- Server validation

Implement on the server:

- Fire cooldown.
- Maximum range.
- Wall blocking.
- Target selection.
- Damage.
- Death and respawn.
- Kills, deaths, and score.

Test:

- A shot cannot pass through a wall.
- A shot cannot damage the shooter.
- A player cannot fire faster than the cooldown.
- Health and score update on both clients.

## Milestone 10: Add Lobby and Rounds

Learn:

- State machines
- Timers and `Instant`
- Match lifecycle
- State-transition validation

Implement these phases:

```rust
enum GamePhase {
    Lobby,
    Playing,
}
```

Add:

- Lobby countdown
- Minimum player rule
- Map voting
- Round start
- Match duration
- Match-over standings
- Return to lobby

Test:

- A player can join the lobby.
- The countdown begins at the chosen minimum.
- The round starts exactly once.
- Late joins are handled intentionally.
- The match ends and returns to the lobby.

## Milestone 11: Add Chat and Ping

Learn:

- Message routing
- Rate limiting
- Round-trip time
- Basic input validation

Implement:

- Chat message broadcast.
- Maximum chat length.
- Ping timestamps.
- Pong response.
- Latency display.

Test:

- Both clients see a chat message.
- Oversized messages are rejected or truncated.
- Ping updates while the game is running.

## Milestone 12: Improve Network Quality

Learn:

- Sequence numbers
- Client-side interpolation
- Prediction and reconciliation
- Reliable versus unreliable messages
- Lag simulation

Add later:

- Snapshot sequence numbers.
- Input sequence numbers.
- Interpolation for remote players.
- Reliable delivery for joins, votes, and match results.
- Packet-loss and latency testing.
- Server-side rate limits.

Do not start here. These features are much easier after the basic game works.

## Milestone 13: Security and Production Concerns

Learn:

- Trust boundaries
- Input validation
- Denial-of-service risks
- Authentication
- Encryption
- Logging and monitoring

Validate on the server:

- Username length and characters
- Movement values
- Rotation values
- Fire rate
- Message size
- Valid phase transitions
- Maximum players

Never trust the client to report its own position, damage, score, or successful hit.

## Final Learning Checklist

- [ ] Rust ownership and borrowing
- [ ] Cargo and project modules
- [ ] UDP networking
- [ ] Serialization with Serde
- [ ] Client/server architecture
- [ ] Fixed-timestep or tick-based simulation
- [ ] Delta time
- [ ] Vectors and trigonometry
- [ ] Grid algorithms
- [ ] Procedural maze generation
- [ ] Collision detection
- [ ] Raycasting
- [ ] State machines
- [ ] Timers and cooldowns
- [ ] Health, damage, and respawning
- [ ] Snapshot replication
- [ ] Interpolation and prediction
- [ ] Validation and security
- [ ] Testing multiplayer behavior

## A Good First Personal Version

Keep your first version small:

- One small maze
- One weapon
- Two players
- No map voting
- No skins
- No chat
- One five-minute round
- Simple colored rectangles instead of detailed art

Once that works, add one feature at a time. The networking and server-authority design are the important lessons; the menus and visual effects can come later.
