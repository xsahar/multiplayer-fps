use std::collections::HashMap;
use std::env;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use maze_wars::shared::collision::move_with_collision;
use maze_wars::shared::player::Player;
use maze_wars::shared::protocol::{ClientMessage, PlayerState, ServerMessage};
use maze_wars::shared::world::{level_by_index, World};

const LOBBY_DURATION_SECS: u64 = 20;

pub struct Server {
    socket: UdpSocket,
    players: HashMap<SocketAddr, Player>,
    inputs: HashMap<SocketAddr, (f32, f32)>,
    last_seen: HashMap<SocketAddr, Instant>,
    last_shot: HashMap<SocketAddr, Instant>,
    level_votes: HashMap<SocketAddr, u8>,
    level: u8,
    lobby_deadline: Option<Instant>,
    match_started: bool,
    tick: u64,
    next_id: u32,
}

fn spawn_for_player(world: &World, player_id: u32, offset: u32) -> (f32, f32) {
    if world.spawn_points.is_empty() {
        return (1.5, 1.5);
    }
    let index = if offset == 0 && player_id == 1 {
        0
    } else if offset == 0 && player_id == 2 {
        world.spawn_points.len() - 1
    } else {
        (player_id as usize + offset as usize - 1) % world.spawn_points.len()
    };
    world.spawn_points[index]
}

fn select_level(votes: impl IntoIterator<Item = u8>, fallback: u8) -> u8 {
    let mut counts = [0usize; 4];
    for level in votes {
        if (1..=3).contains(&level) {
            counts[level as usize] += 1;
        }
    }
    let highest = (1..=3)
        .map(|level| counts[level as usize])
        .max()
        .unwrap_or(0);
    let leaders: Vec<u8> = (1..=3)
        .filter(|level| counts[*level as usize] == highest)
        .collect();
    if highest > 0 && leaders.len() == 1 {
        leaders[0]
    } else {
        fallback
    }
}

impl Server {
    pub fn new() -> std::io::Result<Self> {
        Self::new_with_addr("0.0.0.0:8080")
    }

    pub fn new_with_addr(addr: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            players: HashMap::new(),
            inputs: HashMap::new(),
            last_seen: HashMap::new(),
            last_shot: HashMap::new(),
            level_votes: HashMap::new(),
            level: 1,
            lobby_deadline: None,
            match_started: false,
            tick: 0,
            next_id: 1,
        })
    }

    fn send_to(&self, addr: SocketAddr, message: &ServerMessage) -> std::io::Result<()> {
        let payload = bincode::serialize(message)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.socket.send_to(&payload, addr)?;
        Ok(())
    }

    fn snapshot(&self) -> ServerMessage {
        let players = self
            .players
            .values()
            .map(|p| PlayerState {
                id: p.id,
                username: p.username.clone(),
                level: p.level,
                x: p.x,
                y: p.y,
                angle: p.angle,
                kills: p.kills,
                deaths: p.deaths,
            })
            .collect();

        ServerMessage::Snapshot {
            tick: self.tick,
            players,
        }
    }

    fn apply_level(&mut self, level: u8) {
        self.level = level.clamp(1, 3);
        let world = level_by_index(self.level);
        for player in self.players.values_mut() {
            let spawn = spawn_for_player(&world, player.id, 0);
            player.level = self.level;
            player.x = spawn.0;
            player.y = spawn.1;
            player.angle = 0.0;
        }
    }

    fn waiting_message(&self, now: Instant, addr: SocketAddr) -> ServerMessage {
        let seconds_left = self
            .lobby_deadline
            .map(|deadline| deadline.saturating_duration_since(now).as_secs())
            .unwrap_or(0)
            .min(u8::MAX as u64) as u8;
        ServerMessage::Waiting {
            connected: self.players.len(),
            required: 2,
            level: self.level,
            vote: self.level_votes.get(&addr).copied().unwrap_or(0),
            seconds_left,
        }
    }

    fn handle_shot(&mut self, shooter_addr: SocketAddr) {
        let now = Instant::now();
        if self
            .last_shot
            .get(&shooter_addr)
            .is_some_and(|last| now.duration_since(*last) < Duration::from_millis(250))
        {
            return;
        }
        self.last_shot.insert(shooter_addr, now);

        let Some(shooter) = self.players.get(&shooter_addr) else {
            return;
        };
        let shooter_id = shooter.id;
        let shooter_level = shooter.level;
        let shooter_x = shooter.x;
        let shooter_y = shooter.y;
        let shooter_angle = shooter.angle;
        let level = level_by_index(shooter_level);
        let direction_x = shooter_angle.cos();
        let direction_y = shooter_angle.sin();
        let mut hit: Option<(SocketAddr, f32)> = None;

        for (target_addr, target) in &self.players {
            if target.id == shooter_id || target.level != shooter_level {
                continue;
            }

            let to_target_x = target.x - shooter_x;
            let to_target_y = target.y - shooter_y;
            let distance = (to_target_x * to_target_x + to_target_y * to_target_y).sqrt();
            if distance > 20.0 || distance <= 0.01 {
                continue;
            }

            let forward_distance = to_target_x * direction_x + to_target_y * direction_y;
            let sideways_distance = (to_target_x * direction_y - to_target_y * direction_x).abs();
            if forward_distance <= 0.0 || sideways_distance > 0.45 {
                continue;
            }

            let mut visible = true;
            let samples = (distance / 0.08).ceil() as usize;
            for sample in 1..samples {
                let progress = sample as f32 / samples as f32;
                let ray_x = shooter_x + to_target_x * progress;
                let ray_y = shooter_y + to_target_y * progress;
                let cell_x = ray_x.floor() as usize;
                let cell_y = ray_y.floor() as usize;
                if cell_y >= level.tiles.len()
                    || cell_x >= level.tiles[cell_y].len()
                    || level.tiles[cell_y][cell_x] == 1
                {
                    visible = false;
                    break;
                }
            }

            if visible && hit.map_or(true, |(_, closest)| distance < closest) {
                hit = Some((*target_addr, distance));
            }
        }

        let Some((target_addr, _)) = hit else {
            return;
        };
        if let Some(shooter) = self.players.get_mut(&shooter_addr) {
            shooter.kills += 1;
        }
        if let Some(target) = self.players.get_mut(&target_addr) {
            target.deaths += 1;
            let spawn = spawn_for_player(&level, target.id, target.deaths);
            target.x = spawn.0;
            target.y = spawn.1;
            target.angle = 0.0;
        }
    }

    pub fn run(&mut self) {
        let mut buffer = [0u8; 2048];
        let mut last_broadcast = Instant::now();

        loop {
            loop {
                match self.socket.recv_from(&mut buffer) {
                    Ok((len, addr)) => {
                        if let Ok(message) = bincode::deserialize::<ClientMessage>(&buffer[..len]) {
                            match message {
                                ClientMessage::Hello {
                                    username,
                                    protocol_version: _,
                                } => {
                                    let world = level_by_index(self.level);
                                    let spawn = spawn_for_player(&world, self.next_id, 0);
                                    let player = Player::new(
                                        self.next_id,
                                        &username,
                                        spawn.0,
                                        spawn.1,
                                        self.level,
                                    );
                                    self.next_id += 1;
                                    self.players.insert(addr, player.clone());
                                    self.inputs.insert(addr, (0.0, 0.0));
                                    self.last_seen.insert(addr, Instant::now());
                                    if self.players.len() == 2 && self.lobby_deadline.is_none() {
                                        self.lobby_deadline = Some(
                                            Instant::now()
                                                + Duration::from_secs(LOBBY_DURATION_SECS),
                                        );
                                    }
                                    let _ = self.send_to(
                                        addr,
                                        &ServerMessage::Welcome {
                                            player_id: player.id,
                                            level: self.level,
                                        },
                                    );
                                    println!("Connected {} as player {}", username, player.id);
                                }
                                ClientMessage::Input {
                                    sequence: _,
                                    turn,
                                    move_forward,
                                } => {
                                    self.inputs.insert(addr, (turn, move_forward));
                                    self.last_seen.insert(addr, Instant::now());
                                }
                                ClientMessage::Shoot { sequence: _ } => {
                                    self.handle_shot(addr);
                                    self.last_seen.insert(addr, Instant::now());
                                }
                                ClientMessage::ChangeLevel { level } => {
                                    if self.players.contains_key(&addr) && !self.match_started {
                                        self.level_votes.insert(addr, level.clamp(1, 3));
                                        self.last_seen.insert(addr, Instant::now());
                                    }
                                }
                                ClientMessage::Disconnect => {
                                    self.players.remove(&addr);
                                    self.inputs.remove(&addr);
                                    self.last_seen.remove(&addr);
                                    self.last_shot.remove(&addr);
                                    self.level_votes.remove(&addr);
                                    if self.players.is_empty() {
                                        self.lobby_deadline = None;
                                    }
                                    println!("Disconnected client {}", addr);
                                }
                            }
                        }
                    }
                    Err(ref err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }

            self.tick += 1;
            if last_broadcast.elapsed() >= Duration::from_millis(50) {
                let now = Instant::now();
                let stale_clients: Vec<SocketAddr> = self
                    .last_seen
                    .iter()
                    .filter_map(|(addr, seen)| {
                        (now.duration_since(*seen) > Duration::from_secs(3)).then_some(*addr)
                    })
                    .collect();
                for addr in stale_clients {
                    self.players.remove(&addr);
                    self.inputs.remove(&addr);
                    self.last_seen.remove(&addr);
                    self.last_shot.remove(&addr);
                    self.level_votes.remove(&addr);
                }

                if self.players.len() < 2 {
                    if self.match_started {
                        self.match_started = false;
                        self.level_votes.clear();
                        self.lobby_deadline = if self.players.is_empty() {
                            None
                        } else {
                            Some(now + Duration::from_secs(LOBBY_DURATION_SECS))
                        };
                    }
                    for addr in self.players.keys().copied().collect::<Vec<_>>() {
                        let waiting = self.waiting_message(now, addr);
                        let payload = bincode::serialize(&waiting).unwrap();
                        let _ = self.socket.send_to(&payload, addr);
                    }
                    last_broadcast = Instant::now();
                    continue;
                }

                if !self.match_started {
                    if !self.lobby_deadline.is_some_and(|deadline| now >= deadline) {
                        for addr in self.players.keys().copied().collect::<Vec<_>>() {
                            let waiting = self.waiting_message(now, addr);
                            let payload = bincode::serialize(&waiting).unwrap();
                            let _ = self.socket.send_to(&payload, addr);
                        }
                        last_broadcast = Instant::now();
                        continue;
                    }

                    let selected_level =
                        select_level(self.level_votes.values().copied(), self.level);
                    self.apply_level(selected_level);
                    self.match_started = true;
                    self.lobby_deadline = None;
                }

                for (addr, player) in self.players.iter_mut() {
                    let (turn, move_forward) = self.inputs.get(addr).copied().unwrap_or((0.0, 0.0));
                    let turn = turn.clamp(-1.0, 1.0);
                    player.angle += turn * 0.06;
                    let speed = move_forward * 0.10;
                    let dx = speed * player.angle.cos();
                    let dy = speed * player.angle.sin();
                    let level = level_by_index(player.level);
                    let next = move_with_collision(
                        (player.x, player.y),
                        (player.x + dx, player.y + dy),
                        &level.tiles,
                        0.35,
                    );
                    player.x = next.0;
                    player.y = next.1;
                }

                let snapshot = self.snapshot();
                let payload = bincode::serialize(&snapshot).unwrap();
                for addr in self.players.keys().copied().collect::<Vec<_>>() {
                    let _ = self.socket.send_to(&payload, addr);
                }
                last_broadcast = Instant::now();
            }
        }
    }
}

fn main() {
    let bind_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());
    let mut server = Server::new_with_addr(&bind_addr).expect("Failed to start server");
    println!("Server listening on {}", bind_addr);
    server.run();
}
