use std::net::UdpSocket;
use std::time::{Duration, Instant};

use macroquad::prelude::*;

use maze_wars::shared::protocol::{ClientMessage, ServerMessage};
use maze_wars::shared::world::level_by_index;

struct ConnectionForm {
    server_ip: String,
    username: String,
    level: u8,
    focused_field: u8,
    error: String,
}

impl Default for ConnectionForm {
    fn default() -> Self {
        Self {
            server_ip: "127.0.0.1:8080".to_string(),
            username: String::new(),
            level: 1,
            focused_field: 0,
            error: String::new(),
        }
    }
}

impl ConnectionForm {
    fn update(&mut self) -> Option<(String, String, u8)> {
        if is_key_pressed(KeyCode::Tab) {
            self.focused_field = (self.focused_field + 1) % 3;
        }
        if self.focused_field == 2 {
            if is_key_pressed(KeyCode::Key1) {
                self.level = 1;
            }
            if is_key_pressed(KeyCode::Key2) {
                self.level = 2;
            }
            if is_key_pressed(KeyCode::Key3) {
                self.level = 3;
            }
            if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::Down) {
                self.level = if self.level == 1 { 3 } else { self.level - 1 };
            }
            if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::Up) {
                self.level = if self.level == 3 { 1 } else { self.level + 1 };
            }
        } else if is_key_pressed(KeyCode::Backspace) {
            if self.focused_field == 0 {
                self.server_ip.pop();
            } else {
                self.username.pop();
            }
        }
        while let Some(character) = get_char_pressed() {
            if self.focused_field < 2 && !character.is_control() {
                if self.focused_field == 0 {
                    if self.server_ip.len() < 64 {
                        self.server_ip.push(character);
                    }
                } else if self.username.len() < 64 {
                    self.username.push(character);
                }
            }
        }

        if is_key_pressed(KeyCode::Enter) {
            if self.server_ip.trim().is_empty() || self.username.trim().is_empty() {
                self.error = "Enter both a server address and username".to_string();
            } else {
                return Some((
                    self.server_ip.trim().to_string(),
                    self.username.trim().to_string(),
                    self.level,
                ));
            }
        }
        None
    }
}

fn draw_connection_form(form: &ConnectionForm) {
    clear_background(Color::new(0.08, 0.12, 0.15, 1.0));
    let center_x = screen_width() * 0.5 - 220.0;
    draw_text("MAZE WARS", center_x, 150.0, 44.0, WHITE);
    draw_text(
        "Connect to a Maze Wars server",
        center_x,
        184.0,
        20.0,
        LIGHTGRAY,
    );

    draw_text("SERVER IP:PORT", center_x, 245.0, 18.0, WHITE);
    draw_rectangle_lines(
        center_x,
        260.0,
        440.0,
        48.0,
        2.0,
        if form.focused_field == 0 { GREEN } else { GRAY },
    );
    draw_text(&form.server_ip, center_x + 14.0, 292.0, 22.0, WHITE);

    draw_text("USERNAME", center_x, 350.0, 18.0, WHITE);
    draw_rectangle_lines(
        center_x,
        365.0,
        440.0,
        48.0,
        2.0,
        if form.focused_field == 1 { GREEN } else { GRAY },
    );
    draw_text(&form.username, center_x + 14.0, 397.0, 22.0, WHITE);

    draw_text(
        "TAB: switch field    ENTER: connect",
        center_x,
        465.0,
        18.0,
        LIGHTGRAY,
    );
    draw_text(
        &format!("LEVEL: {}    Press 1, 2, or 3 to choose", form.level),
        center_x,
        500.0,
        18.0,
        if form.focused_field == 2 {
            GREEN
        } else {
            LIGHTGRAY
        },
    );
    if !form.error.is_empty() {
        draw_text(&form.error, center_x, 535.0, 18.0, RED);
    }
}

struct ClientRuntime {
    socket: UdpSocket,
    username: String,
    state: Option<ServerMessage>,
    player_id: Option<u32>,
    sequence: u64,
    move_value: f32,
    turn_value: f32,
    last_fps: f64,
    fps_elapsed: f32,
    fps_frames: u32,
    last_input: Instant,
    shot_flash_until: Option<Instant>,
}

impl ClientRuntime {
    fn new(server_ip: &str, username: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        socket.connect(server_ip)?;
        Ok(Self {
            socket,
            username: username.to_string(),
            state: None,
            player_id: None,
            sequence: 0,
            move_value: 0.0,
            turn_value: 0.0,
            last_fps: 0.0,
            fps_elapsed: 0.0,
            fps_frames: 0,
            last_input: Instant::now(),
            shot_flash_until: None,
        })
    }

    fn hello(&self) -> std::io::Result<()> {
        let msg = ClientMessage::Hello {
            username: self.username.clone(),
            protocol_version: 1,
        };
        let payload = bincode::serialize(&msg).unwrap();
        self.socket.send(&payload)?;
        Ok(())
    }

    fn change_level(&self, level: u8) {
        let message = ClientMessage::ChangeLevel { level };
        let _ = self.socket.send(&bincode::serialize(&message).unwrap());
    }

    fn poll_server(&mut self) {
        let mut buffer = [0u8; 4096];
        loop {
            match self.socket.recv(&mut buffer) {
                Ok(len) => {
                    if let Ok(message) = bincode::deserialize::<ServerMessage>(&buffer[..len]) {
                        match &message {
                            ServerMessage::Welcome { player_id, .. } => {
                                self.player_id = Some(*player_id);
                            }
                            ServerMessage::Snapshot { .. } => {
                                self.state = Some(message);
                            }
                            ServerMessage::Waiting { .. } => {
                                self.state = Some(message);
                            }
                            ServerMessage::Error(_) => {}
                        }
                    }
                }
                Err(ref err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
    }

    fn send_input(&mut self) {
        if self.player_id.is_none() || self.last_input.elapsed() < Duration::from_millis(50) {
            return;
        }

        self.last_input = Instant::now();
        self.sequence += 1;
        let msg = ClientMessage::Input {
            sequence: self.sequence,
            turn: self.turn_value,
            move_forward: self.move_value,
        };
        let payload = bincode::serialize(&msg).unwrap();
        let _ = self.socket.send(&payload);
    }

    fn shoot(&mut self) {
        if self.player_id.is_none() {
            return;
        }
        self.sequence += 1;
        let message = ClientMessage::Shoot {
            sequence: self.sequence,
        };
        let _ = self.socket.send(&bincode::serialize(&message).unwrap());
        self.shot_flash_until = Some(Instant::now() + Duration::from_millis(90));
    }

    fn update_input(&mut self) {
        let mut move_value = 0.0;
        let mut turn_value = 0.0;

        let forward_pressed = is_key_down(KeyCode::W) || is_key_down(KeyCode::Up);
        let backward_pressed = is_key_down(KeyCode::S) || is_key_down(KeyCode::Down);
        let left_pressed = is_key_down(KeyCode::A) || is_key_down(KeyCode::Left);
        let right_pressed = is_key_down(KeyCode::D) || is_key_down(KeyCode::Right);

        if forward_pressed && !backward_pressed {
            move_value = 1.0;
        }
        if backward_pressed && !forward_pressed {
            move_value = -1.0;
        }
        if left_pressed && !right_pressed {
            turn_value = -1.0;
        }
        if right_pressed && !left_pressed {
            turn_value = 1.0;
        }

        self.move_value = move_value;
        self.turn_value = turn_value;

        if is_key_pressed(KeyCode::Space) || is_mouse_button_pressed(MouseButton::Left) {
            self.shoot();
        }

        if is_key_pressed(KeyCode::Q) {
            let msg = ClientMessage::Disconnect;
            let _ = self.socket.send(&bincode::serialize(&msg).unwrap());
            std::process::exit(0);
        }
    }
}

fn draw_maze_and_players(runtime: &ClientRuntime) {
    let sky_color = Color::new(0.18, 0.24, 0.29, 1.0);
    let floor_color = Color::new(0.16, 0.15, 0.13, 1.0);
    clear_background(sky_color);
    draw_rectangle(
        0.0,
        screen_height() * 0.48,
        screen_width(),
        screen_height() * 0.52,
        floor_color,
    );

    let map = vec![
        vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        vec![1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        vec![1, 0, 1, 1, 1, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1],
        vec![1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 1],
        vec![1, 0, 1, 0, 1, 1, 1, 1, 1, 0, 1, 1, 0, 1, 0, 1],
        vec![1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1],
        vec![1, 1, 1, 0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 1, 0, 1],
        vec![1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 1],
        vec![1, 0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 0, 1, 0, 1],
        vec![1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1],
        vec![1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 1, 0, 1],
        vec![1, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 1],
        vec![1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 1, 0, 1, 0, 1],
        vec![1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        vec![1, 0, 1, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1],
        vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    ];
    let current_level = runtime
        .player_id
        .and_then(|player_id| {
            runtime.state.as_ref().and_then(|state| match state {
                ServerMessage::Snapshot { players, .. } => players
                    .iter()
                    .find(|player| player.id == player_id)
                    .map(|player| player.level),
                _ => None,
            })
        })
        .unwrap_or(1);
    let map = map_for_level(current_level, &map);
    let (kills, deaths) = runtime
        .player_id
        .and_then(|player_id| {
            runtime.state.as_ref().and_then(|state| match state {
                ServerMessage::Snapshot { players, .. } => players
                    .iter()
                    .find(|player| player.id == player_id)
                    .map(|player| (player.kills, player.deaths)),
                _ => None,
            })
        })
        .unwrap_or((0, 0));

    let cell = 8.0;
    let map_width = map.iter().map(Vec::len).max().unwrap_or(0) as f32 * cell;
    let map_height = map.len() as f32 * cell;
    let offset_x = (screen_width() - map_width - 16.0).max(16.0);
    let offset_y = 48.0;
    let player_x = runtime
        .player_id
        .and_then(|_| runtime.state.as_ref())
        .and_then(|state| match state {
            ServerMessage::Snapshot { players, .. } => players
                .iter()
                .find(|p| Some(p.id) == runtime.player_id)
                .map(|p| p.x),
            _ => None,
        })
        .unwrap_or(1.5);
    let player_y = runtime
        .player_id
        .and_then(|_| runtime.state.as_ref())
        .and_then(|state| match state {
            ServerMessage::Snapshot { players, .. } => players
                .iter()
                .find(|p| Some(p.id) == runtime.player_id)
                .map(|p| p.y),
            _ => None,
        })
        .unwrap_or(1.5);
    let player_angle = runtime
        .player_id
        .and_then(|_| runtime.state.as_ref())
        .and_then(|state| match state {
            ServerMessage::Snapshot { players, .. } => players
                .iter()
                .find(|p| Some(p.id) == runtime.player_id)
                .map(|p| p.angle),
            _ => None,
        })
        .unwrap_or(0.0);

    draw_first_person(&map, player_x, player_y, player_angle);
    draw_visible_opponents(runtime, &map, player_x, player_y, player_angle);
    if runtime
        .shot_flash_until
        .is_some_and(|until| Instant::now() < until)
    {
        draw_circle(
            screen_width() * 0.5,
            40.0 + (screen_height() - 88.0) * 0.48,
            18.0,
            Color::new(1.0, 0.65, 0.15, 0.8),
        );
    }
    draw_rectangle(
        offset_x - 6.0,
        offset_y - 6.0,
        map_width + 12.0,
        map_height + 12.0,
        Color::new(0.04, 0.07, 0.09, 0.92),
    );
    for (y, row) in map.iter().enumerate() {
        for (x, cell_value) in row.iter().enumerate() {
            let px = offset_x + x as f32 * cell;
            let py = offset_y + y as f32 * cell;
            if *cell_value == 1 {
                draw_rectangle(px, py, cell, cell, DARKGRAY);
            } else {
                draw_rectangle(px, py, cell, cell, LIGHTGRAY);
            }
        }
    }

    let player_px = offset_x + player_x * cell;
    let player_py = offset_y + player_y * cell;
    draw_circle(player_px, player_py, 4.0, GREEN);

    if let Some(ServerMessage::Snapshot { players, .. }) = &runtime.state {
        for p in players {
            if Some(p.id) == runtime.player_id || p.level != current_level {
                continue;
            }
            let px = offset_x + (p.x as f32) * cell;
            let py = offset_y + (p.y as f32) * cell;
            draw_circle(px, py, 4.0, RED);
        }
    }

    draw_rectangle(12.0, 8.0, 220.0, 100.0, Color::new(0.04, 0.07, 0.09, 0.90));
    draw_rectangle_lines(
        12.0,
        8.0,
        220.0,
        100.0,
        2.0,
        Color::new(0.35, 0.52, 0.58, 1.0),
    );
    draw_text("MAZE WARS", 24.0, 30.0, 26.0, WHITE);
    draw_text(
        &format!("Player: {}", runtime.username),
        24.0,
        55.0,
        18.0,
        WHITE,
    );
    draw_text(
        &format!("FPS: {:.1}", runtime.last_fps),
        24.0,
        78.0,
        18.0,
        WHITE,
    );
    draw_text(
        &format!("KILLS: {}  DEATHS: {}", kills, deaths),
        24.0,
        98.0,
        16.0,
        WHITE,
    );
    draw_text(
        "WASD move, SPACE/CLICK fire, Q quit",
        24.0,
        screen_height() - 20.0,
        18.0,
        WHITE,
    );
}

fn draw_waiting_screen(
    runtime: &ClientRuntime,
    connected: usize,
    required: usize,
    level: u8,
    vote: u8,
    seconds_left: u8,
) {
    clear_background(Color::new(0.08, 0.12, 0.15, 1.0));
    let center_x = screen_width() * 0.5 - 240.0;
    draw_text("MAZE WARS", center_x, 190.0, 44.0, WHITE);
    draw_text("LOBBY", center_x, 270.0, 28.0, YELLOW);
    draw_text(
        &format!("Players connected: {} / {}", connected, required),
        center_x,
        320.0,
        22.0,
        LIGHTGRAY,
    );
    draw_text(
        &if vote == 0 {
            format!("Your vote: not submitted  |  Default: level {}", level)
        } else {
            format!("Your vote: level {}  |  Default: level {}", vote, level)
        },
        center_x,
        355.0,
        18.0,
        LIGHTGRAY,
    );
    draw_text(
        &if connected < required {
            "Waiting for another player".to_string()
        } else {
            format!("Match starts in {} seconds", seconds_left)
        },
        center_x,
        400.0,
        18.0,
        WHITE,
    );
    draw_text(
        &format!("Player: {}", runtime.username),
        center_x,
        420.0,
        18.0,
        WHITE,
    );
}

fn draw_first_person(map: &[Vec<i32>], player_x: f32, player_y: f32, player_angle: f32) {
    let view_x = 0.0;
    let view_y = 40.0;
    let view_width = screen_width().max(160.0);
    let view_height = (screen_height() - 88.0).max(180.0);
    let horizon = view_y + view_height * 0.48;

    draw_rectangle(
        view_x,
        view_y,
        view_width,
        view_height * 0.48,
        Color::new(0.18, 0.24, 0.29, 1.0),
    );
    draw_rectangle(
        view_x,
        horizon,
        view_width,
        view_height * 0.52,
        Color::new(0.16, 0.15, 0.13, 1.0),
    );

    let field_of_view = std::f32::consts::FRAC_PI_2;
    let columns = view_width as i32;
    for column in 0..columns {
        let screen_position = column as f32 / columns as f32;
        let ray_angle = player_angle + (screen_position - 0.5) * field_of_view;
        let direction_x = ray_angle.cos();
        let direction_y = ray_angle.sin();
        let mut distance = 0.05;
        let mut wall_distance = 20.0;

        while distance < 20.0 {
            let ray_x = player_x + direction_x * distance;
            let ray_y = player_y + direction_y * distance;
            let map_x = ray_x.floor() as i32;
            let map_y = ray_y.floor() as i32;
            if map_y < 0 || map_y >= map.len() as i32 || map_x < 0 || map_x >= map[0].len() as i32 {
                wall_distance = distance;
                break;
            }
            if map[map_y as usize][map_x as usize] == 1 {
                wall_distance = distance;
                break;
            }
            distance += 0.03;
        }

        let corrected_distance = (wall_distance * (ray_angle - player_angle).cos()).max(0.1);
        let wall_height = (view_height / corrected_distance).min(view_height * 1.4);
        let wall_top = horizon - wall_height * 0.5;
        let brightness = (0.85 - corrected_distance * 0.035).clamp(0.18, 0.85);
        draw_rectangle(
            view_x + column as f32,
            wall_top,
            1.5,
            wall_height,
            Color::new(brightness * 0.42, brightness * 0.48, brightness * 0.52, 1.0),
        );
    }

    draw_line(
        view_x + view_width * 0.5 - 7.0,
        horizon,
        view_x + view_width * 0.5 + 7.0,
        horizon,
        1.0,
        WHITE,
    );
    draw_line(
        view_x + view_width * 0.5,
        horizon - 7.0,
        view_x + view_width * 0.5,
        horizon + 7.0,
        1.0,
        WHITE,
    );
    draw_text("FIELD VIEW", 250.0, 28.0, 18.0, WHITE);
}

fn draw_visible_opponents(
    runtime: &ClientRuntime,
    map: &[Vec<i32>],
    player_x: f32,
    player_y: f32,
    player_angle: f32,
) {
    let Some(ServerMessage::Snapshot { players, .. }) = &runtime.state else {
        return;
    };
    let Some(player_id) = runtime.player_id else {
        return;
    };
    let view_width = screen_width();
    let view_height = screen_height() - 88.0;
    let horizon = 40.0 + view_height * 0.48;
    let fov = std::f32::consts::FRAC_PI_2;

    for opponent in players {
        if opponent.id == player_id {
            continue;
        }
        let delta_x = opponent.x - player_x;
        let delta_y = opponent.y - player_y;
        let distance = (delta_x * delta_x + delta_y * delta_y).sqrt();
        let relative_angle = normalize_angle(delta_y.atan2(delta_x) - player_angle);
        if opponent.level != current_player_level(runtime)
            || distance <= 0.1
            || distance > 20.0
            || relative_angle.abs() > fov * 0.5
            || !has_line_of_sight(map, player_x, player_y, opponent.x, opponent.y)
        {
            continue;
        }

        let screen_x = view_width * 0.5 + relative_angle / fov * view_width;
        let size = (view_height / distance * 0.16).clamp(10.0, 120.0);
        let top = horizon - size;
        draw_rectangle(screen_x - size * 0.28, top, size * 0.56, size, RED);
        draw_circle(screen_x, top - size * 0.18, size * 0.22, MAROON);
    }
}

fn current_player_level(runtime: &ClientRuntime) -> u8 {
    runtime
        .player_id
        .and_then(|player_id| {
            runtime.state.as_ref().and_then(|state| match state {
                ServerMessage::Snapshot { players, .. } => players
                    .iter()
                    .find(|player| player.id == player_id)
                    .map(|player| player.level),
                _ => None,
            })
        })
        .unwrap_or(1)
}

fn normalize_angle(angle: f32) -> f32 {
    let mut normalized = angle;
    while normalized > std::f32::consts::PI {
        normalized -= std::f32::consts::TAU;
    }
    while normalized < -std::f32::consts::PI {
        normalized += std::f32::consts::TAU;
    }
    normalized
}

fn has_line_of_sight(map: &[Vec<i32>], start_x: f32, start_y: f32, end_x: f32, end_y: f32) -> bool {
    let distance = ((end_x - start_x).powi(2) + (end_y - start_y).powi(2)).sqrt();
    let samples = (distance / 0.08).ceil() as usize;
    for sample in 1..samples {
        let progress = sample as f32 / samples as f32;
        let x = start_x + (end_x - start_x) * progress;
        let y = start_y + (end_y - start_y) * progress;
        let cell_x = x.floor() as usize;
        let cell_y = y.floor() as usize;
        if cell_y >= map.len() || cell_x >= map[cell_y].len() || map[cell_y][cell_x] == 1 {
            return false;
        }
    }
    true
}

fn map_for_level(level: u8, default_map: &[Vec<i32>]) -> Vec<Vec<i32>> {
    if level == 1 {
        return default_map.to_vec();
    }

    level_by_index(level)
        .tiles
        .into_iter()
        .map(|row| row.into_iter().map(i32::from).collect())
        .collect()
}

#[macroquad::main("Maze Wars")]
async fn main() {
    let mut form = ConnectionForm::default();
    let mut runtime: Option<ClientRuntime> = None;
    let mut last_frame = Instant::now();

    loop {
        if runtime.is_none() {
            if let Some((server_ip, username, level)) = form.update() {
                match ClientRuntime::new(&server_ip, &username) {
                    Ok(client) => {
                        if client.hello().is_ok() {
                            client.change_level(level);
                            runtime = Some(client);
                            form.error.clear();
                        } else {
                            form.error = "Could not send connection request".to_string();
                        }
                    }
                    Err(error) => {
                        form.error = format!("Connection failed: {}", error);
                    }
                }
            }
            draw_connection_form(&form);
            next_frame().await;
            continue;
        }

        let runtime = runtime.as_mut().expect("runtime created above");
        runtime.poll_server();

        // 1. If no state has arrived from server yet, show a connecting screen
        if runtime.state.is_none() {
            clear_background(Color::new(0.08, 0.12, 0.15, 1.0));
            let center_x = screen_width() * 0.5 - 200.0;
            draw_text("MAZE WARS", center_x, 220.0, 44.0, WHITE);
            draw_text("Connecting to server...", center_x, 300.0, 22.0, YELLOW);
            draw_text("Press Q to cancel", center_x, 360.0, 18.0, LIGHTGRAY);
            
            if is_key_pressed(KeyCode::Q) {
                std::process::exit(0);
            }
            next_frame().await;
            continue;
        }

        // 2. If server sent a Waiting packet, display the lobby
        let waiting = runtime.state.as_ref().and_then(|state| match state {
            ServerMessage::Waiting {
                connected,
                required,
                level,
                vote,
                seconds_left,
            } => Some((*connected, *required, *level, *vote, *seconds_left)),
            _ => None,
        });

        if let Some((connected, required, level, vote, seconds_left)) = waiting {
            runtime.move_value = 0.0;
            runtime.turn_value = 0.0;
            runtime.send_input();
            draw_waiting_screen(runtime, connected, required, level, vote, seconds_left);
            next_frame().await;
            continue;
        }

        // 3. Game in progress: process player inputs and draw maze
        runtime.update_input();
        if runtime.player_id.is_some() {
            runtime.send_input();
        }

        let dt = last_frame.elapsed().as_secs_f32();
        last_frame = Instant::now();
        runtime.fps_elapsed += dt;
        runtime.fps_frames += 1;
        if runtime.fps_elapsed >= 0.5 {
            runtime.last_fps = runtime.fps_frames as f64 / runtime.fps_elapsed as f64;
            runtime.fps_elapsed = 0.0;
            runtime.fps_frames = 0;
        }

        draw_maze_and_players(&runtime);
        next_frame().await;
    }
}
