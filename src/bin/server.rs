use multiplayer_fps::common::{raycast_to_player, Maze, Player};
use multiplayer_fps::protocol::{ClientMessage, ServerMessage};
use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GamePhase {
    Lobby,
    Playing,
}

fn main() {
    let socket = UdpSocket::bind("127.0.0.1:34254")
        .expect("Could not bind server socket");
    socket
        .set_nonblocking(true)
        .expect("Could not set UDP socket nonblocking");

    let mut clients: HashMap<SocketAddr, u32> = HashMap::new();
    let mut usernames: HashSet<String> = HashSet::new();
    let mut players: HashMap<SocketAddr, Player> = HashMap::new();
    let maze = Maze::simple();
    let mut next_player_id = 1;
    let mut game_phase = GamePhase::Lobby;
    let mut lobby_countdown = 10u32;
    let mut match_time_remaining = 30u32;
    let mut last_tick = Instant::now();

    println!("Server starting...");
    println!("Server listening on 127.0.0.1:34254");

    let mut buffer = [0u8; 1024];

    println!("Waiting for UDP clients...");

    loop {
        if game_phase == GamePhase::Lobby && !players.is_empty() {
            if last_tick.elapsed() >= Duration::from_secs(1) {
                lobby_countdown = lobby_countdown.saturating_sub(1);
                last_tick = Instant::now();
                broadcast_lobby_state(&socket, &clients, &usernames, lobby_countdown);
                println!("Lobby countdown: {}", lobby_countdown);

                if lobby_countdown == 0 {
                    game_phase = GamePhase::Playing;
                    match_time_remaining = 30;
                    reset_players_for_match(&mut players, &maze);
                    broadcast_snapshots(&socket, &clients, &players);
                    println!("Match started");
                }
            }
        } else if game_phase == GamePhase::Playing {
            if last_tick.elapsed() >= Duration::from_secs(1) {
                match_time_remaining = match_time_remaining.saturating_sub(1);
                last_tick = Instant::now();
                println!("Match time remaining: {}", match_time_remaining);

                if match_time_remaining == 0 {
                    game_phase = GamePhase::Lobby;
                    lobby_countdown = 10;
                    reset_players_for_match(&mut players, &maze);
                    broadcast_lobby_state(&socket, &clients, &usernames, lobby_countdown);
                    broadcast_snapshots(&socket, &clients, &players);
                    println!("Round ended; lobby restarted");
                }
            }
        }

        let Ok((amount, sender)) = socket.recv_from(&mut buffer) else {
            continue;
        };

        println!("Received {} bytes from {}", amount, sender);

        let (message, _bytes_read): (ClientMessage, usize) =
            match bincode::serde::decode_from_slice(
                &buffer[..amount],
                bincode::config::standard(),
            ) {
                Ok(message) => message,
                Err(error) => {
                    println!("Could not decode client message: {}", error);
                    continue;
                }
            };

        match message {
            ClientMessage::Join { username } => {
                if usernames.contains(&username) {
                    send_message(
                        &socket,
                        sender,
                        &ServerMessage::JoinRejected {
                            reason: String::from("Username already taken"),
                        },
                    );
                    println!("Rejected duplicate username: {}", username);
                    continue;
                }

                usernames.insert(username.clone());

                let player_id = next_player_id;
                next_player_id += 1;

                let mut player = Player::new(username.clone(), 0.0, 0.0);
                if let Some((spawn_x, spawn_y)) = maze.find_spawn() {
                    player.x = spawn_x as f32;
                    player.y = spawn_y as f32;
                }

                clients.insert(sender, player_id);
                players.insert(sender, player);

                send_message(
                    &socket,
                    sender,
                    &ServerMessage::JoinAccepted { player_id },
                );

                broadcast_snapshots(&socket, &clients, &players);
                broadcast_lobby_state(&socket, &clients, &usernames, lobby_countdown);

                println!("{} joined with ID {}", username, player_id);
                println!("Connected clients: {}", clients.len());
                println!("Current phase: {:?}", game_phase);
            }
            ClientMessage::Input { change_x, change_y } => {
                let Some(player_id) = clients.get(&sender).copied() else {
                    println!("Ignoring input from unknown client {}", sender);
                    continue;
                };

                let Some(player) = players.get_mut(&sender) else {
                    continue;
                };

                if !player.alive {
                    println!("Ignoring input from dead player {}", player.name);
                    continue;
                }

                let moved = maze.move_player(
                    player,
                    change_x as isize,
                    change_y as isize,
                );

                println!("Input from {}: moved={}", player.name, moved);

                if game_phase == GamePhase::Playing {
                    broadcast_snapshots(&socket, &clients, &players);
                }

                let _ = player_id;
            }
            ClientMessage::Ping { id } => {
                send_message(
                    &socket,
                    sender,
                    &ServerMessage::Pong { id },
                );
                println!("Pong sent to {} for ping {}", sender, id);
            }
            ClientMessage::Fire { angle } => {
                handle_fire(
                    &socket,
                    sender,
                    angle,
                    game_phase,
                    &clients,
                    &mut players,
                    &maze,
                );
            }
        }
    }
}

fn handle_fire(
    socket: &UdpSocket,
    sender: SocketAddr,
    angle: f32,
    game_phase: GamePhase,
    clients: &HashMap<SocketAddr, u32>,
    players: &mut HashMap<SocketAddr, Player>,
    maze: &Maze,
) {
    if game_phase != GamePhase::Playing {
        return;
    }

    let Some(shooter_id) = clients.get(&sender).copied() else {
        println!("Ignoring fire from unknown client {}", sender);
        return;
    };

    let Some(shooter) = players.get(&sender) else {
        return;
    };

    if !shooter.alive {
        println!("Ignoring fire from dead player {}", shooter.name);
        return;
    }

    let start_x = shooter.x;
    let start_y = shooter.y;
    let shooter_name = shooter.name.clone();

    let targets: Vec<(u32, f32, f32)> = players
        .iter()
        .filter_map(|(addr, player)| {
            if *addr == sender || !player.alive {
                return None;
            }
            let player_id = *clients.get(addr)?;
            Some((player_id, player.x, player.y))
        })
        .collect();

    let Some((target_id, distance)) = raycast_to_player(maze, start_x, start_y, angle, &targets)
    else {
        println!("Player {} fired at angle {} and missed", shooter_name, angle);
        return;
    };

    let Some(target_addr) = clients
        .iter()
        .find_map(|(addr, id)| (*id == target_id).then_some(*addr))
    else {
        return;
    };

    let Some(target) = players.get_mut(&target_addr) else {
        return;
    };

    let died = target.apply_hit();
    let health = target.health;
    let target_name = target.name.clone();

    if died {
        if let Some(shooter) = players.get_mut(&sender) {
            shooter.kills += 1;
        }
    }

    broadcast_message(
        socket,
        clients,
        &ServerMessage::PlayerHit {
            shooter_id,
            target_id,
            health,
        },
    );

    if died {
        broadcast_message(
            socket,
            clients,
            &ServerMessage::PlayerDied {
                player_id: target_id,
                killer_id: shooter_id,
            },
        );
        println!(
            "Player {} killed {} at distance {:.2}",
            shooter_name, target_name, distance
        );
    } else {
        println!(
            "Player {} hit {} for 10 HP ({} remaining, distance {:.2})",
            shooter_name, target_name, health, distance
        );
    }
}

fn reset_players_for_match(players: &mut HashMap<SocketAddr, Player>, maze: &Maze) {
    for player in players.values_mut() {
        let (spawn_x, spawn_y) = maze.find_spawn().unwrap_or((1, 1));
        player.respawn_for_round(spawn_x as f32, spawn_y as f32);
    }
}

fn broadcast_lobby_state(
    socket: &UdpSocket,
    clients: &HashMap<SocketAddr, u32>,
    usernames: &HashSet<String>,
    countdown: u32,
) {
    let players: Vec<String> = usernames.iter().cloned().collect();

    broadcast_message(
        socket,
        clients,
        &ServerMessage::LobbyState {
            players,
            countdown,
        },
    );
}

fn broadcast_snapshots(
    socket: &UdpSocket,
    clients: &HashMap<SocketAddr, u32>,
    players: &HashMap<SocketAddr, Player>,
) {
    for recipient in clients.keys() {
        for (client_addr, player) in players {
            let Some(player_id) = clients.get(client_addr) else {
                continue;
            };

            send_message(
                socket,
                *recipient,
                &ServerMessage::Snapshot {
                    player_id: *player_id,
                    x: player.x,
                    y: player.y,
                    health: player.health,
                    alive: player.alive,
                },
            );
        }
    }
}

fn broadcast_message(
    socket: &UdpSocket,
    clients: &HashMap<SocketAddr, u32>,
    message: &ServerMessage,
) {
    for recipient in clients.keys() {
        send_message(socket, *recipient, message);
    }
}

fn send_message(socket: &UdpSocket, recipient: SocketAddr, message: &ServerMessage) {
    let bytes = bincode::serde::encode_to_vec(message, bincode::config::standard())
        .expect("Could not encode server message");

    socket
        .send_to(&bytes, recipient)
        .expect("Could not send server message");
}
