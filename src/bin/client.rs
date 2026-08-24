use macroquad::prelude::*;
use multiplayer_fps::common::Maze;
use multiplayer_fps::protocol::{ClientMessage, ServerMessage};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

fn read_server_message(socket: &UdpSocket, buffer: &mut [u8]) -> Option<ServerMessage> {
    let (amount, _) = socket.recv_from(buffer).ok()?;
    let (message, _) = bincode::serde::decode_from_slice(
        &buffer[..amount],
        bincode::config::standard(),
    )
    .ok()?;

    Some(message)
}

fn wall_distance(maze: &Maze, start_x: f32, start_y: f32, angle: f32) -> f32 {
    let direction_x = angle.cos();
    let direction_y = angle.sin();
    let mut distance = 0.0;

    while distance < 20.0 {
        let ray_x = start_x + direction_x * distance;
        let ray_y = start_y + direction_y * distance;
        let tile_x = ray_x.floor() as isize;
        let tile_y = ray_y.floor() as isize;

        if tile_x < 0
            || tile_y < 0
            || maze.is_wall(tile_x as usize, tile_y as usize)
        {
            return distance.max(0.01);
        }

        distance += 0.02;
    }

    20.0
}

fn draw_raycast_view(maze: &Maze, player_position: (f32, f32), player_angle: f32) {
    let view_width = screen_width() * 0.68;
    let view_height = screen_height();
    let field_of_view = std::f32::consts::FRAC_PI_2;

    draw_rectangle(0.0, 0.0, view_width, view_height * 0.5, color_u8!(35, 45, 60, 255));
    draw_rectangle(
        0.0,
        view_height * 0.5,
        view_width,
        view_height * 0.5,
        color_u8!(55, 48, 42, 255),
    );

    let column_width = 2.0;
    let mut screen_x = 0.0;
    while screen_x < view_width {
        let column_ratio = screen_x / view_width;
        let ray_angle = player_angle + (column_ratio - 0.5) * field_of_view;
        let raw_distance = wall_distance(
            maze,
            player_position.0,
            player_position.1,
            ray_angle,
        );
        let distance = (raw_distance * (ray_angle - player_angle).cos()).max(0.01);
        let wall_height = (view_height / distance).min(view_height);
        let wall_top = (view_height - wall_height) * 0.5;
        let shade = (220.0 / (1.0 + distance * 0.35)).clamp(35.0, 220.0) as u8;

        draw_rectangle(
            screen_x,
            wall_top,
            column_width,
            wall_height,
            color_u8!(shade, shade, shade, 255),
        );
        screen_x += column_width;
    }

    draw_line(
        view_width * 0.5 - 8.0,
        view_height * 0.5,
        view_width * 0.5 + 8.0,
        view_height * 0.5,
        2.0,
        WHITE,
    );
    draw_line(
        view_width * 0.5,
        view_height * 0.5 - 8.0,
        view_width * 0.5,
        view_height * 0.5 + 8.0,
        2.0,
        WHITE,
    );
}

fn draw_maze(maze: &Maze, players: &HashMap<u32, (f32, f32)>, my_player_id: Option<u32>) {
    let tile_size = 18.0;
    let offset_x = screen_width() * 0.72;
    let offset_y = 90.0;
    let map_width = maze.width as f32 * tile_size;
    let map_height = maze.height as f32 * tile_size;

    draw_rectangle(
        offset_x - 8.0,
        offset_y - 8.0,
        map_width + 16.0,
        map_height + 16.0,
        color_u8!(20, 25, 30, 220),
    );

    for (y, row) in maze.tiles.iter().enumerate() {
        for (x, cell) in row.iter().enumerate() {
            let px = offset_x + x as f32 * tile_size;
            let py = offset_y + y as f32 * tile_size;
            let color = if *cell == '#' {
                color_u8!(60, 60, 70, 255)
            } else {
                color_u8!(90, 110, 90, 255)
            };
            draw_rectangle(px, py, tile_size, tile_size, color);
        }
    }

    for (player_id, (x, y)) in players {
        let px = offset_x + x * tile_size + tile_size * 0.5;
        let py = offset_y + y * tile_size + tile_size * 0.5;
        let color = if Some(*player_id) == my_player_id {
            color_u8!(120, 220, 120, 255)
        } else {
            color_u8!(220, 120, 120, 255)
        };
        draw_circle(px, py, 7.0, color);
    }

    draw_text("Multiplayer FPS", 20.0, 30.0, 24.0, WHITE);
    draw_text("Map", offset_x, offset_y - 12.0, 18.0, WHITE);
}

fn draw_hud(players: &HashMap<u32, (f32, f32)>, player_angle: f32) {
    draw_text(&format!("FPS: {}", get_fps()), 20.0, 58.0, 20.0, WHITE);
    draw_text(
        &format!("Players: {}", players.len()),
        20.0,
        82.0,
        20.0,
        WHITE,
    );
    draw_text(
        &format!("Angle: {:.2}", player_angle),
        20.0,
        106.0,
        20.0,
        WHITE,
    );
}

#[macroquad::main("Multiplayer FPS")]
async fn main() {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("Could not bind client socket");
    socket
        .set_nonblocking(true)
        .expect("Could not set client socket nonblocking");

    let join_message = ClientMessage::Join {
        username: String::from("Sahar"),
    };
    let join_bytes = bincode::serde::encode_to_vec(
        &join_message,
        bincode::config::standard(),
    )
    .expect("Could not encode join message");

    socket
        .send_to(&join_bytes, "127.0.0.1:34254")
        .expect("Could not send join message");

    let mut buffer = [0u8; 1024];
    let mut players: HashMap<u32, (f32, f32)> = HashMap::new();
    let mut my_player_id = None;
    let mut joined = false;
    let mut next_input_at = Instant::now();
    let mut player_angle = 0.0;
    let maze = Maze::simple();

    while !joined {
        if let Some(message) = read_server_message(&socket, &mut buffer) {
            match message {
                ServerMessage::JoinAccepted { player_id } => {
                    my_player_id = Some(player_id);
                    players.insert(player_id, (0.0, 0.0));
                    joined = true;
                    println!("Join accepted: {}", player_id);
                }
                ServerMessage::JoinRejected { reason } => {
                    println!("Rejected: {}", reason);
                    return;
                }
                ServerMessage::Snapshot { player_id, x, y } => {
                    players.insert(player_id, (x, y));
                }
                ServerMessage::LobbyState { players: _, countdown } => {
                    println!("Lobby countdown: {}", countdown);
                }
                ServerMessage::Pong { id } => {
                    println!("Pong for {}", id);
                }
            }
        }
    }

    loop {
        let rotation_speed = 2.5 * get_frame_time();
        if is_key_down(KeyCode::A) {
            player_angle -= rotation_speed;
        }
        if is_key_down(KeyCode::D) {
            player_angle += rotation_speed;
        }

        if next_input_at.elapsed() >= Duration::from_millis(200) {
            let input = ClientMessage::Input {
                change_x: 1,
                change_y: 0,
            };
            let input_bytes = bincode::serde::encode_to_vec(
                &input,
                bincode::config::standard(),
            )
            .expect("Could not encode input message");

            socket
                .send_to(&input_bytes, "127.0.0.1:34254")
                .expect("Could not send input message");
            next_input_at = Instant::now();
        }

        if let Ok((amount, _)) = socket.recv_from(&mut buffer) {
            let (message, _) = bincode::serde::decode_from_slice(
                &buffer[..amount],
                bincode::config::standard(),
            )
            .expect("Could not decode snapshot");

            match message {
                ServerMessage::Snapshot { player_id, x, y } => {
                    players.insert(player_id, (x, y));
                }
                ServerMessage::LobbyState { players: _, countdown } => {
                    println!("Lobby countdown: {}", countdown);
                }
                ServerMessage::Pong { id } => {
                    println!("Pong for {}", id);
                }
                ServerMessage::JoinAccepted { player_id } => {
                    my_player_id = Some(player_id);
                    players.insert(player_id, (0.0, 0.0));
                }
                ServerMessage::JoinRejected { reason } => {
                    println!("Rejected: {}", reason);
                }
            }
        }

        clear_background(BLACK);
        let player_position = my_player_id
            .and_then(|player_id| players.get(&player_id).copied())
            .unwrap_or((1.5, 1.5));
        draw_raycast_view(&maze, player_position, player_angle);
        draw_maze(&maze, &players, my_player_id);
        draw_hud(&players, player_angle);
        next_frame().await;
    }
}
#[cfg(test)]
mod tests {
    use super::wall_distance;
    use multiplayer_fps::common::Maze;

    #[test]
    fn ray_hits_the_east_wall() {
        let maze = Maze::simple();
        let distance = wall_distance(&maze, 1.5, 1.5, 0.0);

        assert!((distance - 2.5).abs() < 0.03);
    }

    #[test]
    fn ray_hits_the_north_wall_after_rotation() {
        let maze = Maze::simple();
        let distance = wall_distance(&maze, 1.5, 1.5, -std::f32::consts::FRAC_PI_2);

        assert!((distance - 0.5).abs() < 0.03);
    }
}
