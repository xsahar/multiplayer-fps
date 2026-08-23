use multiplayer_fps::common::{Maze, Player};
use multiplayer_fps::protocol::{ClientMessage, ServerMessage};
use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, UdpSocket};

fn main() {
    let socket = UdpSocket::bind("127.0.0.1:34254")
        .expect("Could not bind server socket");

    let mut clients: HashMap<SocketAddr, u32> = HashMap::new();
    let mut usernames: HashSet<String> = HashSet::new();
    let mut players: HashMap<SocketAddr, Player> = HashMap::new();
    let maze = Maze::simple();
    let mut next_player_id = 1;
        
    println!("Server starting...");
    println!("Server listening on 127.0.0.1:34254");

    let mut buffer = [0u8; 1024];

    println!("Waiting for UDP clients...");

    loop {
        let (amount, sender) = socket
            .recv_from(&mut buffer)
            .expect("Could not receive UDP packet");

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
                if usernames.contains(&username){
                    send_message(
                        &socket,
                        sender,
                        &ServerMessage::JoinRejected{
                            reason: String::from("USername already taken"),
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

                println!("{} joined with ID {}", username, player_id);
                println!("Connected clients: {}", clients.len());
            }
            ClientMessage::Input { change_x, change_y } => {
                let Some(player_id) = clients.get(&sender).copied() else {
                    println!("Ignoring input from unknown client {}", sender);
                    continue;
                };

                let Some(player) = players.get_mut(&sender) else {
                    continue;
                };

                let moved = maze.move_player(
                    player,
                    change_x as isize,
                    change_y as isize,
                );

                println!("Input from {}: moved={}", player.name, moved);

                send_message(
                    &socket,
                    sender,
                    &ServerMessage::Snapshot {
                        player_id,
                        x: player.x,
                        y: player.y,
                    },
                );
            }
        }
    }
}

fn send_message(socket: &UdpSocket, recipient: SocketAddr, message: &ServerMessage) {
    let bytes = bincode::serde::encode_to_vec(message, bincode::config::standard())
        .expect("Could not encode server message");

    socket
        .send_to(&bytes, recipient)
        .expect("Could not send server message");
}
