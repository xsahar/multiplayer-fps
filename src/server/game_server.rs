use crate::common::maze::{level_dims, Maze};
use crate::common::player::Player;
use crate::common::protocol::*;
use rand::prelude::*;
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

const INACTIVE_SECS: u64 = 6; // remove clients that stop pinging/sending for >6s
const HIT_DAMAGE: i32 = 22; // tuned TTK
const FIRE_COOLDOWN_MS: u64 = 150;
const MAX_RANGE: f32 = 30.0;
const HIT_RADIUS: f32 = 0.30;
const SPAWN_SAFE_RADIUS: f32 = 6.0; // tiles
const SPAWN_PROTECTION_MS: u64 = 3000; // 3 seconds invulnerability after spawn

// NEW: treat players as circles in collision to prevent wall clipping
const PLAYER_RADIUS: f32 = 0.28; // ~28% of a tile; tweak if you want a “thicker” feel

// ---- username validator (simple, local) ----
fn is_valid_username(name: &str) -> bool {
    let len = name.chars().count();
    if len < 3 || len > 16 {
        return false;
    }
    name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ---- per-level match durations ----
#[inline]
fn match_duration_for(level: LevelKind) -> Duration {
    match level {
        LevelKind::Easy => Duration::from_secs(180),   // 3 min
        LevelKind::Normal => Duration::from_secs(300), // 5 min
        LevelKind::Hard => Duration::from_secs(600),   // 10 min
    }
}

// ---- per-level maze "braiding": knock down extra walls to cut dead ends and
// open up sightlines/shortcuts. Only Easy gets this treatment; Normal/Hard stay
// as tighter, fully "perfect" mazes.
#[inline]
fn braid_chance_for(level: LevelKind) -> f32 {
    match level {
        LevelKind::Easy => 0.45,
        LevelKind::Normal => 0.0,
        LevelKind::Hard => 0.0,
    }
}

fn generate_level_maze(level: LevelKind) -> Maze {
    let (w, h) = level_dims(level);
    let mut maze = Maze::generate_prims(w, h, rand::thread_rng());
    maze.braid(&mut rand::thread_rng(), braid_chance_for(level));
    maze
}

pub struct GameServer {
    socket: UdpSocket,
    phase: GamePhase,
    level: LevelKind,
    maze: Maze,
    players: HashMap<u32, Player>,
    endpoints: HashMap<u32, SocketAddr>,
    ids_by_addr: HashMap<SocketAddr, u32>,
    next_id: u32,
    rng: ThreadRng,
    lobby_end_at: Instant,
    votes: HashMap<u32, MapChoice>,
    match_end_at: Option<Instant>,
    last_seen: HashMap<u32, Instant>,
}

impl GameServer {
    pub fn bind(addr: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;

        let level = LevelKind::Normal;
        let maze = generate_level_maze(level);

        Ok(Self {
            socket,
            phase: GamePhase::Lobby,
            level,
            maze,
            players: HashMap::new(),
            endpoints: HashMap::new(),
            ids_by_addr: HashMap::new(),
            next_id: 1,
            rng: rand::thread_rng(),
            // When server boots, wait for players (no countdown yet)
            lobby_end_at: Instant::now() + Duration::from_secs(999_999),
            votes: HashMap::new(),
            match_end_at: None,
            last_seen: HashMap::new(),
        })
    }

    fn broadcast(&self, msg: &ServerToClient) {
        let buf = bincode::serialize(msg).unwrap();
        for addr in self.endpoints.values() {
            let _ = self.socket.send_to(&buf, addr);
        }
    }

    fn send_to(&self, addr: SocketAddr, msg: &ServerToClient) {
        let buf = bincode::serialize(msg).unwrap();
        let _ = self.socket.send_to(&buf, addr);
    }

    fn current_lobby_ms_left(&self) -> u64 {
        if self.phase != GamePhase::Lobby {
            return 0;
        }
        self.lobby_end_at
            .saturating_duration_since(Instant::now())
            .as_millis() as u64
    }

    fn vote_counts(&self) -> [u32; 4] {
        let mut counts = [0u32; 4];
        for v in self.votes.values() {
            match v {
                MapChoice::Map1 => counts[0] += 1,
                MapChoice::Map2 => counts[1] += 1,
                MapChoice::Map3 => counts[2] += 1,
                MapChoice::Random => counts[3] += 1,
            }
        }
        counts
    }

    fn mark_seen_addr(&mut self, addr: SocketAddr) {
        if let Some(pid) = self.ids_by_addr.get(&addr).copied() {
            self.last_seen.insert(pid, Instant::now());
        }
    }

    fn is_valid_username_server(&self, name: &str, addr: SocketAddr) -> Result<(), String> {
        if !is_valid_username(name) {
            return Err("Invalid username. Use 3-16 chars [A-Z a-z 0-9 _]".to_string());
        }
        // no duplicates (case-insensitive), but allow same address to rejoin
        let lower = name.to_ascii_lowercase();
        for p in self.players.values() {
            if p.name.to_ascii_lowercase() == lower {
                // Check if it's the same address trying to rejoin
                if let Some(&existing_addr) = self.endpoints.get(&p.id) {
                    if existing_addr != addr {
                        return Err("Username already in use".to_string());
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_join(&mut self, addr: SocketAddr, name: String, skin: PlayerSkin) {
        println!(
            "[Server] handle_join: {} from {:?}, phase={:?}",
            name, addr, self.phase
        );

        // Remove existing player from this address if any
        if let Some(&old_id) = self.ids_by_addr.get(&addr) {
            println!("[Server] Removing old player id={}", old_id);
            self.players.remove(&old_id);
            self.endpoints.remove(&old_id);
            self.ids_by_addr.remove(&addr);
            self.last_seen.remove(&old_id);
            self.votes.remove(&old_id);
        }

        if self.players.len() >= MAX_PLAYERS {
            self.send_to(addr, &ServerToClient::ServerFull);
            return;
        }

        if let Err(reason) = self.is_valid_username_server(&name, addr) {
            println!("[Server] Username validation failed: {}", reason);
            self.send_to(addr, &ServerToClient::JoinRejected(JoinRejected { reason }));
            return;
        }

        let id = self.next_id;
        self.next_id += 1;

        let (sx, sy) = self.pick_safe_spawn();
        let mut p = Player::new(id, name.clone(), sx, sy, skin);
        p.yaw = rand::Rng::gen_range(&mut self.rng, 0.0..std::f32::consts::TAU);
        p.spawn_protection_until = now_ms() + SPAWN_PROTECTION_MS; // 3s protection

        self.players.insert(id, p);
        self.endpoints.insert(id, addr);
        self.ids_by_addr.insert(addr, id);
        self.last_seen.insert(id, Instant::now());

        println!(
            "[Server] Player {} added with id={}, total players={}",
            name,
            id,
            self.players.len()
        );

        match self.phase {
            GamePhase::Lobby => {
                self.votes.entry(id).or_insert(MapChoice::Random);

                // Start/hold timer based on player count
                if self.players.len() >= 2 {
                    // Start timer if it was paused
                    if self.lobby_end_at.saturating_duration_since(Instant::now())
                        > Duration::from_secs(100_000)
                    {
                        self.lobby_end_at = Instant::now() + Duration::from_secs(20);
                        println!("[Server] Timer started: 20s (2+ players reached)");
                    }
                } else {
                    // Pause the timer while waiting for more players
                    self.lobby_end_at = Instant::now() + Duration::from_secs(999_999);
                    println!("[Server] Waiting for more players");
                }

                let accept = JoinAccept {
                    player_id: id,
                    width: self.maze.width,
                    height: self.maze.height,
                    level: self.level,
                    tiles: self.maze.tiles.clone(),
                    phase: GamePhase::Lobby,
                    lobby_ms_left: self.current_lobby_ms_left(),
                };

                println!(
                    "[Server] Sending JoinOk with lobby_ms_left={}",
                    accept.lobby_ms_left
                );
                self.send_to(addr, &ServerToClient::JoinOk(accept));

                self.broadcast_lobby_state();
            }
            GamePhase::Playing => {
                // Reject joins during active match
                self.players.remove(&id);
                self.endpoints.remove(&id);
                self.ids_by_addr.remove(&addr);
                self.last_seen.remove(&id);

                self.send_to(
                    addr,
                    &ServerToClient::JoinRejected(JoinRejected {
                        reason: "Match in progress. Please wait for next round.".to_string(),
                    }),
                );
            }
        }
    }

    fn set_level_and_respawn_all(&mut self, level: LevelKind) {
        self.level = level;
        self.maze = generate_level_maze(level);

        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            let (sx, sy) = self.pick_safe_spawn();
            if let Some(p) = self.players.get_mut(&id) {
                p.reset(sx, sy);
                p.yaw = rand::Rng::gen_range(&mut self.rng, 0.0..std::f32::consts::TAU);
                p.reset_stats();
                p.spawn_protection_until = now_ms() + SPAWN_PROTECTION_MS;
            }
        }
    }

    fn announce_level_changed(&self) {
        let accept = JoinAccept {
            player_id: 0,
            width: self.maze.width,
            height: self.maze.height,
            level: self.level,
            tiles: self.maze.tiles.clone(),
            phase: self.phase,
            lobby_ms_left: 0,
        };
        self.broadcast(&ServerToClient::LevelChanged(accept));
    }

    // ==== MOVEMENT: circle-vs-grid collision to prevent wall clipping ====

    /// Circle-rectangle overlap test vs 1x1 wall tile at (tx, ty)
    #[inline]
    fn circle_hits_wall(maze: &Maze, x: f32, y: f32, r: f32, tx: i32, ty: i32) -> bool {
        if maze.get(tx, ty) != 1 {
            return false;
        }
        // rectangle bounds
        let rx0 = tx as f32;
        let ry0 = ty as f32;
        let rx1 = rx0 + 1.0;
        let ry1 = ry0 + 1.0;

        // closest point on rect to circle center
        let cx = x.clamp(rx0, rx1);
        let cy = y.clamp(ry0, ry1);

        let dx = x - cx;
        let dy = y - cy;
        (dx * dx + dy * dy) < r * r
    }

    /// True if a circle at (x, y) with radius r does not intersect any wall.
    fn is_free_circle(maze: &Maze, x: f32, y: f32, r: f32) -> bool {
        let min_tx = (x - r).floor() as i32;
        let max_tx = (x + r).floor() as i32;
        let min_ty = (y - r).floor() as i32;
        let max_ty = (y + r).floor() as i32;

        for ty in min_ty..=max_ty {
            for tx in min_tx..=max_tx {
                if Self::circle_hits_wall(maze, x, y, r, tx, ty) {
                    return false;
                }
            }
        }
        true
    }

    /// Move with axis separation and sliding. Prevents visual and logical overlap with walls.
    fn move_with_collision(maze: &Maze, x: f32, y: f32, dx: f32, dy: f32, r: f32) -> (f32, f32) {
        let mut nx = x;
        let mut ny = y;

        // try X
        let try_x = nx + dx;
        if Self::is_free_circle(maze, try_x, ny, r) {
            nx = try_x;
        } else {
            // slide a bit along Y if blocked in X
        }

        // try Y
        let try_y = ny + dy;
        if Self::is_free_circle(maze, nx, try_y, r) {
            ny = try_y;
        } else {
            // slide a bit along X if blocked in Y (already handled by prior X)
        }

        // tiny corner nudge to avoid sticky corners
        if !Self::is_free_circle(maze, nx, ny, r) {
            // attempt small displacements along axes
            let eps = 0.02;
            let candidates = [
                (nx + eps, ny),
                (nx - eps, ny),
                (nx, ny + eps),
                (nx, ny - eps),
                (x, ny), // fallbacks
                (nx, y),
            ];
            for (ax, ay) in candidates {
                if Self::is_free_circle(maze, ax, ay, r) {
                    return (ax, ay);
                }
            }
            // worst case, stay put
            return (x, y);
        }

        (nx, ny)
    }

    fn step_players(&mut self, dt: f32, inputs: &HashMap<u32, InputState>) {
        let maze: &Maze = &self.maze;
        let mut updates: Vec<(u32, f32, f32, f32)> = Vec::with_capacity(self.players.len());
        let turn_speed: f32 = std::f32::consts::PI * 3.6; // rad/s

        for (id, p) in self.players.iter() {
            let mut nx = p.x;
            let mut ny = p.y;
            let mut yaw = p.yaw;

            if let Some(inp) = inputs.get(&id) {
                // yaw
                yaw += inp.yaw_delta.clamp(-1.0, 1.0) * turn_speed * dt;

                // movement vector
                let f = inp.forward.clamp(-1.0, 1.0);
                let s = inp.strafe.clamp(-1.0, 1.0);

                let dir_x = yaw.cos();
                let dir_y = yaw.sin();
                let strafe_x = -dir_y;
                let strafe_y = dir_x;

                let mut mv_x = dir_x * f + strafe_x * s;
                let mut mv_y = dir_y * f + strafe_y * s;

                let mag = (mv_x * mv_x + mv_y * mv_y).sqrt();
                if mag > 1.0 {
                    mv_x /= mag;
                    mv_y /= mag;
                }

                let speed = p.speed * dt;
                let dx = mv_x * speed;
                let dy = mv_y * speed;

                // NEW: circle-vs-grid collision
                (nx, ny) = Self::move_with_collision(maze, nx, ny, dx, dy, PLAYER_RADIUS);
            }

            updates.push((*id, nx, ny, yaw));
        }

        for (id, nx, ny, yaw) in updates {
            if let Some(p) = self.players.get_mut(&id) {
                p.x = nx;
                p.y = ny;
                p.yaw = normalize_angle(yaw);
            }
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            time_ms: now_ms(),
            players: self
                .players
                .values()
                .map(|p| PlayerNet {
                    id: p.id,
                    name: p.name.clone(),
                    x: p.x,
                    y: p.y,
                    yaw: p.yaw,
                    hp: p.hp,
                    skin: p.skin,
                    kills: p.kills,
                    deaths: p.deaths,
                    score: p.score,
                })
                .collect(),
            game_ms_left: self.current_game_ms_left(),
        }
    }

    pub fn run(&mut self) {
        println!(
            "[Server] Running. UDP: {}",
            self.socket.local_addr().unwrap()
        );
        let mut buf = [0u8; 65535];

        let tick = Duration::from_millis(1000 / SERVER_TICK_HZ as u64);
        let mut last = Instant::now();
        let mut acc = Duration::ZERO;

        let mut last_snapshot = Instant::now();
        let snapshot_dt = Duration::from_millis(1000 / SNAPSHOT_HZ as u64);

        let mut inputs: HashMap<u32, InputState> = HashMap::new();

        // lobby broadcast pacing
        let mut last_lobby_bcast = Instant::now();

        loop {
            // ---- recv
            while let Ok((amt, src)) = self.socket.recv_from(&mut buf) {
                if amt == 0 {
                    break;
                }
                if let Ok(msg) = bincode::deserialize::<ClientToServer>(&buf[..amt]) {
                    // mark this address as alive for keep-alive
                    self.mark_seen_addr(src);

                    match msg {
                        ClientToServer::Join(req) => {
                            self.handle_join(src, req.username, req.skin);
                        }
                        ClientToServer::Input(inp) => {
                            if let Some(pid) = self.ids_by_addr.get(&src) {
                                inputs.insert(*pid, inp.clone());
                            }
                        }
                        ClientToServer::Vote(choice) => {
                            if self.phase == GamePhase::Lobby {
                                if let Some(pid) = self.ids_by_addr.get(&src) {
                                    self.votes.insert(*pid, choice);
                                }
                            }
                        }
                        ClientToServer::Ping(ts) => {
                            // quick pong for latency measurement
                            self.send_to(src, &ServerToClient::Pong(ts));
                        }
                        ClientToServer::ReturnToLobby(username, skin) => {
                            println!(
                                "[Server] ReturnToLobby received: {} from {:?}",
                                username, src
                            );
                            if self.phase == GamePhase::Lobby {
                                self.handle_join(src, username, skin);
                            } else {
                                println!("[Server] Rejected - not in lobby phase");
                            }
                        }
                        ClientToServer::ChatMessage(msg) => {
                            if let Some(&pid) = self.ids_by_addr.get(&src) {
                                if let Some(player) = self.players.get(&pid) {
                                    let chat_msg = ChatMsg {
                                        player_name: player.name.clone(),
                                        message: msg,
                                        timestamp: now_ms(),
                                    };
                                    println!(
                                        "[Chat] {}: {}",
                                        chat_msg.player_name, chat_msg.message
                                    );
                                    self.broadcast(&ServerToClient::ChatMessage(chat_msg));
                                }
                            }
                        }
                    }
                }
            }

            // ---- sweep inactive players
            self.sweep_inactive(&mut inputs);

            // ---- phase logic (LOBBY)
            if self.phase == GamePhase::Lobby {
                // NEW: dynamically pause/resume the lobby timer based on player count
                if self.players.len() < 2 {
                    // If the timer is currently counting down (<= ~27.7 hours left), pause it.
                    if self.lobby_end_at.saturating_duration_since(Instant::now())
                        <= Duration::from_secs(100_000)
                    {
                        self.lobby_end_at = Instant::now() + Duration::from_secs(999_999);
                        println!("[Server] Not enough players; pausing lobby timer");
                    }
                } else {
                    // If we have 2+ players and timer is paused (very far future), start 20s countdown.
                    if self.lobby_end_at.saturating_duration_since(Instant::now())
                        > Duration::from_secs(100_000)
                    {
                        self.lobby_end_at = Instant::now() + Duration::from_secs(20);
                        println!("[Server] 2+ players; starting 20s lobby timer");
                    }
                }

                // periodic lobby broadcast (≈6–7/sec)
                if last_lobby_bcast.elapsed() >= Duration::from_millis(150) {
                    last_lobby_bcast = Instant::now();

                    let names: Vec<String> =
                        self.players.values().map(|p| p.name.clone()).collect();
                    let state = LobbyState {
                        time_ms_left: self.current_lobby_ms_left(),
                        votes: self.vote_counts(),
                        players: names,
                    };
                    self.broadcast(&ServerToClient::LobbyState(state));
                }

                // When timer ends and there are 2+ players, pick map and start Playing
                if Instant::now() >= self.lobby_end_at && self.players.len() >= 2 {
                    let counts = self.vote_counts();
                    let max = counts.iter().copied().max().unwrap_or(0);

                    let winner = if max == 0 {
                        MapChoice::Random
                    } else {
                        let mut tops = Vec::new();
                        for (i, c) in counts.iter().enumerate() {
                            if *c == max {
                                tops.push(i);
                            }
                        }
                        if tops.len() > 1 {
                            MapChoice::Random
                        } else {
                            match tops[0] {
                                0 => MapChoice::Map1,
                                1 => MapChoice::Map2,
                                2 => MapChoice::Map3,
                                _ => MapChoice::Random,
                            }
                        }
                    };

                    let chosen_level = match winner {
                        MapChoice::Map1 => LevelKind::Easy,
                        MapChoice::Map2 => LevelKind::Normal,
                        MapChoice::Map3 => LevelKind::Hard,
                        MapChoice::Random => match self.rng.gen_range(0..3) {
                            0 => LevelKind::Easy,
                            1 => LevelKind::Normal,
                            _ => LevelKind::Hard,
                        },
                    };

                    self.set_level_and_respawn_all(chosen_level);
                    self.phase = GamePhase::Playing;
                    self.votes.clear();

                    // >>> use per-level duration here <<<
                    self.match_end_at = Some(Instant::now() + match_duration_for(self.level));

                    self.announce_level_changed();

                    // Immediately push a first snapshot so clients have live state right away
                    let snap = self.snapshot();
                    self.broadcast(&ServerToClient::Snapshot(snap));
                }
            }

            // ---- round-expiry check (PLAYING -> WINNER -> LOBBY)
            if self.phase == GamePhase::Playing {
                if let Some(end_at) = self.match_end_at {
                    if Instant::now() >= end_at {
                        // 1) compute standings
                        let mut table: Vec<ScoreEntry> = self
                            .players
                            .values()
                            .map(|p| ScoreEntry {
                                id: p.id,
                                name: p.name.clone(),
                                kills: p.kills,
                                deaths: p.deaths,
                                score: p.score,
                            })
                            .collect();
                        table.sort_by(|a, b| b.score.cmp(&a.score).then(b.kills.cmp(&a.kills)));

                        // 2) broadcast winner screen
                        self.broadcast(&ServerToClient::MatchOver { standings: table });

                        // 3) clear all players and reset to empty lobby
                        self.players.clear();
                        self.endpoints.clear();
                        self.ids_by_addr.clear();
                        self.last_seen.clear();
                        self.votes.clear();
                        inputs.clear();

                        self.phase = GamePhase::Lobby;
                        self.match_end_at = None;

                        // Don't start timer - wait for players to join/return
                        self.lobby_end_at = Instant::now() + Duration::from_secs(999_999);
                    }
                }
            }

            // ---- fixed step loop
            let now = Instant::now();
            let dt = now - last;
            last = now;
            acc += dt;

            while acc >= tick {
                if self.phase == GamePhase::Playing {
                    self.step_players(tick.as_secs_f32(), &inputs);
                    self.step_shooting(&inputs);
                }
                acc -= tick;
            }

            // ---- periodic snapshot during Playing
            if self.phase == GamePhase::Playing && last_snapshot.elapsed() >= snapshot_dt {
                last_snapshot = Instant::now();
                let snap = self.snapshot();
                self.broadcast(&ServerToClient::Snapshot(snap));
            }

            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn step_shooting(&mut self, inputs: &HashMap<u32, InputState>) {
        let now = now_ms();

        let shooter_ids: Vec<u32> = self.players.keys().copied().collect();

        for sid in shooter_ids {
            let fire = inputs.get(&sid).map(|i| i.fire).unwrap_or(false);
            if !fire {
                continue;
            }

            let (sx, sy, syaw, can_fire, shooter_addr) =
                match (self.players.get(&sid), self.endpoints.get(&sid)) {
                    (Some(s), Some(addr)) => (
                        s.x,
                        s.y,
                        s.yaw,
                        now.saturating_sub(s.last_shot) >= FIRE_COOLDOWN_MS && s.alive(),
                        *addr,
                    ),
                    _ => continue,
                };
            if !can_fire {
                continue;
            }

            if let Some(s) = self.players.get_mut(&sid) {
                s.last_shot = now;
            }

            let dir_x = syaw.cos();
            let dir_y = syaw.sin();

            let dist_to_wall = self.ray_to_wall_distance(sx, sy, dir_x, dir_y, MAX_RANGE);

            let mut best_t = f32::INFINITY;
            let mut hit_id: Option<u32> = None;

            for (tid, t) in self.players.iter() {
                if *tid == sid || !t.alive() || t.has_spawn_protection(now) {
                    continue;
                }

                let dx = t.x - sx;
                let dy = t.y - sy;

                let t_proj = dx * dir_x + dy * dir_y;
                if t_proj <= 0.0 {
                    continue;
                }
                if t_proj > dist_to_wall || t_proj > MAX_RANGE {
                    continue;
                }

                let perp = (dx * (-dir_y) + dy * dir_x).abs();
                let aim_weight = (t_proj / MAX_RANGE).clamp(0.0, 1.0);
                let effective_radius = HIT_RADIUS * (0.92 + 0.08 * (1.0 - aim_weight));
                if perp <= effective_radius {
                    if t_proj < best_t {
                        best_t = t_proj;
                        hit_id = Some(*tid);
                    }
                }
            }

            if let Some(tid) = hit_id {
                let (rx, ry) = self.pick_safe_spawn();

                let mut killed = false;

                if let Some(target) = self.players.get_mut(&tid) {
                    target.hp -= HIT_DAMAGE;
                    if target.hp <= 0 {
                        target.deaths = target.deaths.saturating_add(1);
                        killed = true;
                        target.reset(rx, ry);
                        target.spawn_protection_until = now + SPAWN_PROTECTION_MS;
                        // 3s protection
                    }
                }

                if let Some(sh) = self.players.get_mut(&sid) {
                    sh.score = sh.score.saturating_add(10);
                    if killed {
                        sh.kills = sh.kills.saturating_add(1);
                    }
                }

                self.send_to(shooter_addr, &ServerToClient::HitConfirm { target_id: tid });
            }
        }
    }

    fn ray_to_wall_distance(
        &self,
        sx: f32,
        sy: f32,
        dir_x: f32,
        dir_y: f32,
        max_range: f32,
    ) -> f32 {
        let mut map_x = sx.floor() as i32;
        let mut map_y = sy.floor() as i32;

        let delta_dist_x = if dir_x.abs() < 1e-6 {
            1e30
        } else {
            (1.0 / dir_x).abs()
        };
        let delta_dist_y = if dir_y.abs() < 1e-6 {
            1e30
        } else {
            (1.0 / dir_y).abs()
        };

        let (step_x, mut side_dist_x) = if dir_x < 0.0 {
            (-1, (sx - map_x as f32) * delta_dist_x)
        } else {
            (1, ((map_x as f32 + 1.0) - sx) * delta_dist_x)
        };
        let (step_y, mut side_dist_y) = if dir_y < 0.0 {
            (-1, (sy - map_y as f32) * delta_dist_y)
        } else {
            (1, ((map_y as f32 + 1.0) - sy) * delta_dist_y)
        };

        let mut dist = max_range;

        for _ in 0..2048 {
            let try_dist = if side_dist_x < side_dist_y {
                side_dist_x += delta_dist_x;
                map_x += step_x;
                (map_x as f32 - sx + (1 - step_x) as f32 * 0.5) / dir_x
            } else {
                side_dist_y += delta_dist_y;
                map_y += step_y;
                (map_y as f32 - sy + (1 - step_y) as f32 * 0.5) / dir_y
            }
            .abs();

            if try_dist > max_range {
                break;
            }

            if self.maze.get(map_x, map_y) == 1 {
                dist = try_dist;
                break;
            }
        }

        dist
    }

    fn broadcast_lobby_state(&self) {
        if self.phase != GamePhase::Lobby {
            return;
        }
        let names: Vec<String> = self.players.values().map(|p| p.name.clone()).collect();
        let state = LobbyState {
            time_ms_left: self.current_lobby_ms_left(),
            votes: self.vote_counts(),
            players: names,
        };
        self.broadcast(&ServerToClient::LobbyState(state));
    }

    fn current_game_ms_left(&self) -> u64 {
        if self.phase != GamePhase::Playing {
            return 0;
        }
        if let Some(end_at) = self.match_end_at {
            end_at.saturating_duration_since(Instant::now()).as_millis() as u64
        } else {
            0
        }
    }

    /// Remove players we haven't heard from recently.
    fn sweep_inactive(&mut self, inputs: &mut HashMap<u32, InputState>) {
        let now = Instant::now();
        let mut removed: Vec<u32> = Vec::new();

        for (&pid, &seen) in self.last_seen.iter() {
            if now.duration_since(seen) >= Duration::from_secs(INACTIVE_SECS) {
                removed.push(pid);
            }
        }

        if removed.is_empty() {
            return;
        }

        for pid in removed {
            if let Some(addr) = self.endpoints.remove(&pid) {
                self.ids_by_addr.remove(&addr);
            }
            self.players.remove(&pid);
            self.votes.remove(&pid);
            self.last_seen.remove(&pid);
            inputs.remove(&pid);
        }

        if self.phase == GamePhase::Lobby {
            self.broadcast_lobby_state();
        }
    }

    fn pick_safe_spawn(&mut self) -> (f32, f32) {
        for _ in 0..50 {
            let (sx, sy) = self.maze.random_spawn(&mut self.rng);
            let mut ok = true;
            for p in self.players.values() {
                let dx = p.x - sx;
                let dy = p.y - sy;
                if (dx * dx + dy * dy).sqrt() < SPAWN_SAFE_RADIUS {
                    ok = false;
                    break;
                }
            }
            if ok {
                return (sx, sy);
            }
        }
        self.maze.random_spawn(&mut self.rng)
    }
}

fn normalize_angle(mut a: f32) -> f32 {
    use std::f32::consts::PI;
    while a > PI {
        a -= 2.0 * PI;
    }
    while a < -PI {
        a += 2.0 * PI;
    }
    a
}

fn now_ms() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
