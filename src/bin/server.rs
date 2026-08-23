use multiplayer_fps::common::{Maze, Player};
use multiplayer_fps::protocol::ClientMessage;
use std::io::{self, Write};
use std::net::UdpSocket;


fn print_maze(maze: &Maze, player: &Player) {
    for (y, row) in maze.tiles.iter().enumerate() {
        for (x, tile) in row.iter().enumerate() {
            if player.x as usize == x && player.y as usize == y {
                print!("P");
            } else {
                print!("{}", tile);
            }
        }
        println!();
    }
}

fn main() {

    let socket = UdpSocket::bind("127.0.0.1:34254")
        .expect("Could not bind server socket");
        
    println!("Server starting...");
    println!("Server listening on 127.0.0.1:34254");

    let mut buffer = [0u8; 1024];

    println!("Waiting for a UDP packet...");

    let (amount, sender) = socket
     .recv_from(&mut buffer)
     .expect("Could not recive Udp packet");

    println!("Received {} bytes from {}", amount, sender);

    let (message, _bytes_read): (ClientMessage, usize) =
        bincode::serde::decode_from_slice(
            &buffer[..amount],
            bincode::config::standard(),
        )
        .expect("Could not decode client message");

    let username = match message {
        ClientMessage::Join { username } => username,
        ClientMessage::Input { .. } => {
            println!("Expected a join message");
            return;
        }
    };

    let mut player = Player::new(username, 0.0, 0.0);

    let maze = Maze::simple();

    if let Some((spawn_x, spawn_y)) = maze.find_spawn(){
        player.x = spawn_x as f32;
        player.y = spawn_y as f32;
    }

    println!("Maze size: {}x{}", maze.width, maze.height);
    println!("{} spawned at ({}, {})", player.name, player.x, player.y);
    print_maze(&maze, &player);

    loop {
    print!("Move with w/a/s/d, or q to quit: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    let command = input.trim();

    if command == "q" {
        break;
    }

    let (direction, change_x, change_y) = match command {
        "w" => ("up", 0, -1),
        "a" => ("left", -1, 0),
        "s" => ("down", 0, 1),
        "d" => ("right", 1, 0),
        _ => {
            println!("Unknown command");
            continue;
        }
    };

    let moved = maze.move_player(&mut player, change_x, change_y);

    if moved {
        println!("Moved {}", direction);
    } else {
        println!("Blocked moving {}", direction);
    }

        println!("Position ({}, {})", player.x, player.y);
        print_maze(&maze, &player);
    }

}
