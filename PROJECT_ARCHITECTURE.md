# Maze Wars - Project Architecture & File Guide

## Overview

**Maze Wars** is a multiplayer first-person shooter (FPS) built in Rust using the Wolfenstein-style raycast rendering technique. Players join a lobby, vote on difficulty/map, then battle in procedurally generated mazes. The game uses UDP networking with a custom binary protocol (bincode serialization).

### Key Features
- Procedurally generated mazes using Prim's algorithm with "braiding"
- 3 difficulty levels: Easy (23×15), Normal (39×27), Hard (55×37)
- 4 playable character skins: Minion, Steve, Cat, Ghost
- Raycast rendering engine (classic Wolfenstein-style)
- Up to 64 concurrent players per server
- Lobby phase with map voting
- Match phase with live scoreboard, spawn protection, and hit detection

---

## Architecture Overview

### Client-Server Model

```
┌─────────────────────────────────────────────────────────────┐
│                        Clients (Players)                     │
├─────────────────────────────────────────────────────────────┤
│  - Capture input (movement, look, fire, vote)               │
│  - Render gameworld via raycasting                          │
│  - Display UI (lobby screen, HUD, scoreboard)               │
│  - Send input to server via UDP                             │
│  - Receive world snapshots & game messages                  │
└────────────────────────┬────────────────────────────────────┘
                         │
                    UDP Port 45000
                         │
┌────────────────────────▼────────────────────────────────────┐
│                    Game Server                               │
├─────────────────────────────────────────────────────────────┤
│  - Authoritative game simulation (30 Hz)                    │
│  - Manages lobby & match phases                             │
│  - Detects hits, handles player movement validation         │
│  - Broadcasts snapshots to all clients (20 Hz)              │
│  - Tracks player kills/deaths/scores                        │
└─────────────────────────────────────────────────────────────┘
```

### Game Flow

```
LOBBY PHASE
  ├─ Player joins & sends their skin choice
  ├─ Server starts 20s timer when 2+ players connected
  ├─ Players vote on map (Easy/Normal/Hard/Random)
  └─ Server picks map based on majority vote

        ↓ Timer expires
        
MATCH PHASE
  ├─ Maze generated, all players spawn at safe locations
  ├─ Players move, look around, fire weapons
  ├─ Hits detected on server, damage applied
  ├─ Kills/deaths/scores tracked
  ├─ Match runs for 3-10 mins depending on difficulty
  └─ Scoreboard shown at round end
  
        ↓ Match ends
        
RETURN TO LOBBY
  └─ Players can vote for next map or leave
```

---

## File Structure & Descriptions

### Root Files

| File | Purpose |
|------|---------|
| `Cargo.toml` | Rust project manifest; declares dependencies (macroquad, bincode, serde, etc.) and binary targets |
| `README.md` | Original project documentation with feature overview and building instructions |
| `PROJECT_ARCHITECTURE.md` | **This file** — detailed architecture guide |

---

### Source Code (`src/`)

#### 1. **`lib.rs`**
   - Library root; re-exports the three main modules
   - Imports: `mod common`, `mod server`, `mod client`

---

#### 2. **`common/` — Shared Code Between Client & Server**

##### **`common/mod.rs`**
   - Module root; exports `protocol`, `maze`, and `player` submodules

##### **`common/protocol.rs`** ⭐ **KEY FILE — Network Communication**
   - Defines all serializable message types using `serde`
   - **Client → Server Messages:**
     - `Join(JoinRequest)` — sent when player enters lobby with username + **skin choice**
     - `Input(InputState)` — sent every frame with movement (forward/strafe/yaw) and fire state
     - `Vote(MapChoice)` — player's map preference (Map1/Map2/Map3/Random)
     - `Ping(u64)` — keepalive; server echoes it back with latency
     - `ReturnToLobby(username, skin)` — rejoin for next round
     - `ChatMessage(String)` — in-game chat
   
   - **Server → Client Messages:**
     - `JoinOk(JoinAccept)` — accepted; includes maze data, player ID, phase
     - `JoinRejected(reason)` — join failed (server full, bad username, etc.)
     - `Snapshot(...)` — world state: all player positions, health, scores, time
     - `LobbyState(...)` — updated lobby info (vote counts, timer)
     - `MatchOver(Standings)` — match ended; show scoreboard
     - `HitConfirm(...)` — shot hit confirmed by server
     - `Pong(u64)` — response to ping (latency measurement)
     - `ServerFull` — connection rejected (64 player limit reached)
     - `ChatMessage(name, text)` — incoming chat from other player

   - **Data Structures:**
     - `JoinRequest { username, skin }` — stores player's chosen character
     - `PlayerSkin enum`:
       - `Minion` — yellow character
       - `Steve` — blocky character
       - `Cat` — feline character
       - `Ghost` — spectral character
     - `LevelKind enum`: `Easy`, `Normal`, `Hard` (difficulty = maze size)
     - `MapChoice enum`: `Map1`, `Map2`, `Map3`, `Random` (what players vote on)
     - `GamePhase enum`: `Lobby`, `Playing`

##### **`common/maze.rs`** — Maze Generation
   - **`Maze struct`**: holds `width`, `height`, `tiles` (Vec<u8>)
     - `0` = empty floor, `1` = wall
   - **Prim's Algorithm** (`generate_prims`):
     - Creates a "perfect" maze (no loops, no dead ends initially)
     - Deterministic algorithm: carves paths by connecting frontier cells
   - **Braiding** (`braid` method):
     - Post-processing: knocks down extra walls to create loops & shortcuts
     - More aggressive on Easy (45% chance), disabled on Normal/Hard (0%)
     - Makes maze more open and playable for beginners
   - **Helper function `level_dims(LevelKind)`**:
     - Returns (width, height) for each difficulty:
       - Easy: 23 × 15
       - Normal: 39 × 27
       - Hard: 55 × 37

##### **`common/player.rs`** — Server-Side Player State
   - **`Player struct`** holds:
     - `id`, `name`, position (`x`, `y`), rotation (`yaw`)
     - `hp` (0–100), `speed` (3.5 tiles/sec)
     - `skin` — player's chosen character
     - `spawn_protection_until` — timestamp for 3-second spawn invulnerability
     - `kills`, `deaths`, `score` — stats tracked per round
     - `last_shot` — cooldown tracking (150 ms between shots)
   - Methods:
     - `new()` — create player at spawn location
     - `alive()` — returns `hp > 0`
     - `reset()` — respawn logic (position, HP reset)
     - `has_spawn_protection()` — check if invulnerable

---

#### 3. **`server/` — Authoritative Game Server**

##### **`server/mod.rs`**
   - Module root; exports `game_server`

##### **`server/game_server.rs`** ⭐ **KEY FILE — Game Loop & Match Logic**
   - **`GameServer struct`** maintains:
     - `phase` (Lobby or Playing)
     - `level` (Easy/Normal/Hard)
     - `maze` (current level's tilemap)
     - `players` — HashMap of all active players
     - `endpoints` — maps player ID → UDP address
     - `ids_by_addr` — maps UDP address → player ID (for routing)
     - `lobby_end_at` — when timer expires
     - `votes` — map preference votes per player
     - `match_end_at` — when current match expires
     - `last_seen` — timestamp of last activity per player (inactivity timeout: 6s)

   - **Key Responsibilities:**
     1. **Player Management**
        - `handle_join()` — new player joins; assigns ID, spawns at safe location
        - Rejects joins during `Playing` phase (⚠️ **This is why your 3rd client couldn't join**)
        - Removes duplicate usernames
     2. **Lobby Phase**
        - Collects map votes from players
        - Starts 20s countdown when 2+ players present
        - Broadcasts lobby state (vote counts, timer)
     3. **Match Phase**
        - Runs 30 Hz tick loop (game simulation)
        - Processes player inputs (movement, rotation, firing)
        - Validates movement (collision with walls)
        - **Hit detection**: raycast from attacker position/yaw, check if hits another player
        - Applies damage, tracks kills/deaths
        - Broadcasts snapshots at 20 Hz
     4. **Round Transitions**
        - Match ends based on timer (`match_duration_for(level)`)
        - Respawns players after death (picks safe spawn location)
        - Returns to lobby for next map

   - **Performance Notes:**
     - `pick_safe_spawn()` — generates random spawns, retries if too close to existing players
     - Uses Rust's `HashMap` for O(1) player lookups
     - Spawning is radius-checked: `SPAWN_SAFE_RADIUS = 6.0` tiles to avoid overlaps

   - **Constants:**
     - `INACTIVE_SECS = 6` — disconnect players not sending packets
     - `HIT_DAMAGE = 22` — damage per hit (tuned for ~4-5 hits to kill)
     - `FIRE_COOLDOWN_MS = 150` — weapon cooldown between shots
     - `SPAWN_PROTECTION_MS = 3000` — 3 seconds invulnerability after spawn

---

#### 4. **`client/` — Client Game Logic & Rendering**

##### **`client/mod.rs`**
   - Module root; exports `network` and `renderer`

##### **`client/network.rs`** — UDP Client Networking
   - **`NetClient struct`**:
     - `socket` — UDP socket (non-blocking)
     - `server` — SocketAddr of the game server
     - `last_ping` — time tracking for keepalive
   
   - **Methods**:
     - `connect()` — bind to local port, set non-blocking mode
     - `join(username, skin)` — send `Join` message with skin choice ✅ **Character selection entry point**
     - `send_input()` — send player input each frame
     - `vote(MapChoice)` — send map preference
     - `poll()` — non-blocking check for incoming server messages
     - `keepalive_tick()` — send Ping every 500ms to stay connected

##### **`client/renderer.rs`** — Graphics & HUD ⭐ **KEY FILE — Rendering Engine**
   - **Raycast Renderer**:
     - `Scene struct` — holds the 3D view rendering state
     - For each column of pixels, cast a ray and find the nearest wall distance
     - Map distance to wall color via `fog()` (applies depth-based color fading)
     - Render walls as vertical strips using the distance values
   
   - **Sprite Rendering** (other players):
     - After walls, render other player sprites on screen
     - Calculate screen X position based on angle-to-player relative to viewer
     - `PlayerDraw struct` — holds data for one player to render
     - Scaling based on distance
   
   - **HUD Elements**:
     - Crosshair (center dot)
     - Compass/minimap showing directions
     - Health/ammo display
     - Scoreboard overlay
     - Kill/death notifications
   
   - **Performance Considerations**:
     - Fog calculation: exponential distance falloff (⚠️ **check if expensive**)
     - Sprite scaling: distance-based LOD
     - Draw calls batched per frame

---

#### 5. **`bin/` — Entry Points**

##### **`bin/server.rs`**
   - Starts `GameServer` on `0.0.0.0:45000`
   - Main loop: 30 Hz tick-based simulation
   - Broadcasts snapshots at 20 Hz
   - Handles incoming UDP packets from clients
   - Runs indefinitely until terminated

##### **`bin/client.rs`** ⭐ **KEY FILE — Main Menu & UI**
   - **Window Config**:
     - Fullscreen by default
     - 60 FPS rendering
   
   - **UI States** (enum `Transition`):
     - `Menu` — main menu with login
     - `Connecting` — attempting to join server
     - `Waiting` — in lobby, waiting for match to start
     - `Game` — in-game play (rendering world, HUD)
     - `Scoreboard` — match-over screen
   
   - **Character Selection** ✅ **Location of skin picker**:
     - Lines ~664–698 in client.rs
     - Displays 4 character cards: Minion, Steve, Cat, Ghost
     - Player clicks to select before joining
     - Shows large preview of selected character
     - Skin selected is sent in `join()` call to server
   
   - **Menu Flow**:
     1. Player enters username
     2. **Player selects character (skin) from 4 cards** ← CHARACTER SELECTION UI
     3. Player clicks "Join Game"
     4. Client sends `Join(JoinRequest { username, skin })`
     5. If successful, enters lobby to wait for match start
   
   - **Rendering**:
     - Menu: card-based UI with neon cyberpunk theme
     - `draw_skin_icon()` — renders small sprite preview of each character
     - `skin_card_button()` — clickable card for skin selection
   
   - **Network Flow**:
     - Connects to server at `127.0.0.1:45000` by default
     - Polls for messages each frame
     - Sends input every frame while in-game

---

## Character Selection Guide

### How to Choose a Character

1. **Start the client**: `cargo run --bin client`
2. **Enter your username** (3–16 alphanumeric + underscore chars)
3. **Four character cards appear**:
   - **Minion** — yellow smiley
   - **Steve** — blocky avatar
   - **Cat** — feline character
   - **Ghost** — spectral form
4. **Click a card** to select (selected card is highlighted in cyan)
5. **Click "Join Game"** — character choice is sent to server
6. Join the lobby and wait for match to start

### Character Skins
- All skins are **functionally identical** (no gameplay differences)
- Purely cosmetic choice for visual identification
- Server stores each player's `skin` choice
- Skins rendered as sprites during gameplay

---

## Known Issues & Analysis

### Issue 1: Only 2 of 3 Clients Appear in Game

**Root Cause**: Clients joining during the **Playing phase are rejected**.

**Flow**:
1. Client 1 joins lobby (Good!)
2. Client 2 joins lobby (Good!)
3. Lobby timer starts (20 seconds)
4. **Timer expires** → Match starts, phase changes to `Playing`
5. Client 3 attempts to join → Server rejects (sees `phase == Playing`)
6. Client 3 stays in connecting state or gets `JoinRejected`

**Location**: [src/server/game_server.rs](src/server/game_server.rs#L217-L225)

**Fix Options**:
- Option A: Allow joining during match (players spawn on next map)
- Option B: Extend lobby timer while players are joining
- Option C: Implement a "spectator" mode for mid-match joins

---

### Issue 2: Character Selection Not Working

**Status**: ✅ **Feature is implemented** but may have UX issues:

**Possible Reasons**:
1. **Character selector UI not visible** → Check if menu phase reached
2. **Clicks not registering** → UI hit detection issue
3. **Skin choice not being sent** → Check network code

**Character Selection Location**: [src/bin/client.rs:664–698](src/bin/client.rs#L664-L698)

**Debug Steps**:
- Ensure you reach the menu (lobby screen)
- Look for 4 character cards
- Try clicking each card (highlighted cyan = selected)
- Check server logs for "handle_join" messages showing the `skin` parameter

---

### Issue 3: Lag & Performance

**Likely Culprits**:

1. **Network latency**: UDP may drop packets; could cause jitter
   - Solution: Implement client-side prediction & interpolation
   
2. **Rendering overhead**:
   - `fog()` calculation per pixel (exponential math)
   - Sprite sorting/drawing for many players
   - Solution: Cache fog values, use lookup table
   
3. **Maze size**:
   - Hard mode = 55×37 grid = complex raycasting
   - Solution: Reduce draw distance or use Level-of-Detail

4. **Server tick rate**:
   - 30 Hz tick but 20 Hz snapshot broadcast
   - Could cause input lag if not interpolated
   - Solution: Increase server tick rate or client-side prediction

**Performance Recommendations**:
- Profile with `cargo build --release` (debug is slow)
- Add FPS counter to HUD
- Monitor network latency via Ping/Pong messages
- Consider using async I/O for server (currently synchronous loop)

---

## Building & Running

### Build

```bash
cargo build --release  # Optimized build (much faster!)
```

### Run Server

```bash
cargo run --release --bin server
# or
cargo run --release --bin server -- 0.0.0.0:45000
```

### Run Client(s)

```bash
cargo run --release --bin client
```

Multiple terminals:
```bash
Terminal 1: cargo run --release --bin server
Terminal 2: cargo run --release --bin client  # Player 1
Terminal 3: cargo run --release --bin client  # Player 2
Terminal 4: cargo run --release --bin client  # Player 3
```

---

## Protocol Diagram

```
┌──────────────┐                                 ┌──────────────┐
│   Client     │                                 │   Server     │
└──────┬───────┘                                 └──────┬───────┘
       │                                                 │
       │ (1) Join(name, skin)                          │
       ├────────────────────────────────────────────────>
       │                                                 │
       │                           (2) JoinOk(id, maze) │
       │<────────────────────────────────────────────────┤
       │                                                 │
       │ (3) Vote(Map1/Map2/Map3/Random)               │
       ├────────────────────────────────────────────────>
       │                                                 │
       │                     (4) LobbyState(...) [timer]│
       │<────────────────────────────────────────────────┤
       │                                                 │
       │ [Repeated: Input, Ping]                        │
       ├────────────────────────────────────────────────>
       │                                                 │
       │                         (5) Snapshot [players] │
       │<────────────────────────────────────────────────┤
       │                                                 │
       │                           (6) HitConfirm [dmg] │
       │<────────────────────────────────────────────────┤
       │                                                 │
       │                         (7) MatchOver [score]  │
       │<────────────────────────────────────────────────┤
       │                                                 │
```

---

## Summary Table

| Component | Role | Entry Point |
|-----------|------|-------------|
| **Server** | Authoritative game loop, match management | `src/bin/server.rs` |
| **Client** | UI, rendering, input capture | `src/bin/client.rs` |
| **Protocol** | Network messages (Join, Input, Snapshot, etc.) | `src/common/protocol.rs` |
| **Maze** | Procedural generation (Prim's + Braiding) | `src/common/maze.rs` |
| **Player** | Server-side player state (HP, position, stats) | `src/common/player.rs` |
| **Network** | UDP socket management, message polling | `src/client/network.rs` |
| **Renderer** | Raycast engine, HUD, sprites | `src/client/renderer.rs` |

---

## Quick Tips

- **Username rules**: 3–16 chars, alphanumeric + underscore
- **Map voting**: Majority wins; "Random" picks any
- **Spawn safety**: 3 seconds of invulnerability after spawning
- **TTK** (Time To Kill): ~4–5 shots (22 damage per hit, 100 HP)
- **Match duration**:
  - Easy: 3 minutes
  - Normal: 5 minutes
  - Hard: 10 minutes

---

*Last updated: 2026-09-01*
