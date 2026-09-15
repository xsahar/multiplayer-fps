// src/bin/client.rs
use maze_wars::client::renderer::col;

use macroquad::prelude::*;
use std::net::SocketAddr;

use maze_wars::client::network::NetClient;
use maze_wars::client::renderer::{CrosshairStyle, Scene};
use maze_wars::common::protocol::*;

/// Custom window configuration: start in fullscreen.
fn window_conf() -> Conf {
    Conf {
        window_title: "Maze Wars".to_owned(),
        fullscreen: true,
        ..Default::default()
    }
}

const UI_BG: Color = Color::new(0.05, 0.05, 0.06, 0.94);
const UI_CARD: Color = Color::new(0.10, 0.10, 0.12, 0.96);
const UI_ACCENT: Color = Color::new(1.00, 0.70, 0.00, 1.00); // glowing amber
const UI_ACCENT2: Color = Color::new(1.00, 0.35, 0.00, 1.00); // deep orange
const UI_TEXT: Color = Color::new(1.00, 0.95, 0.85, 1.00);
const UI_TEXT_DIM: Color = Color::new(0.60, 0.55, 0.50, 1.00);
const UI_BORDER: Color = Color::new(1.00, 0.70, 0.00, 1.00); // amber
const UI_SHADOW: Color = Color::new(0.00, 0.00, 0.00, 0.70);
const UI_DANGER: Color = Color::new(0.95, 0.15, 0.20, 1.00);
const UI_OK: Color = Color::new(0.20, 0.85, 0.45, 1.00);


#[inline]
fn draw_shadow(rect: Rect, spread: f32) {
    draw_rectangle(rect.x + spread, rect.y + spread, rect.w, rect.h, UI_SHADOW);
}

/// Faint expanding outlines to fake a neon glow around an edge.
fn draw_neon_glow(rect: Rect, color: Color, layers: i32) {
    for i in 1..=layers {
        let spread = i as f32 * 2.2;
        let alpha = color.a * (0.14 / i as f32);
        draw_rectangle_lines(
            rect.x - spread,
            rect.y - spread,
            rect.w + spread * 2.0,
            rect.h + spread * 2.0,
            2.0 + spread * 0.5,
            Color::new(color.r, color.g, color.b, alpha),
        );
    }
}

fn draw_card(rect: Rect, thick: f32) {
    draw_shadow(rect, 6.0);
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, UI_CARD);
    draw_neon_glow(rect, UI_BORDER, 3);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, thick, UI_BORDER);
}

fn draw_panel(rect: Rect) {
    draw_shadow(rect, 10.0);
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, UI_BG);
    draw_neon_glow(rect, UI_ACCENT2, 4);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, UI_BORDER);
}

fn ui_title(center_x: f32, y: f32, title: &str, subtitle: Option<&str>) {
    let t = measure_text(title, None, 40, 1.0);
    let tx = center_x - t.width * 0.5;

    // neon glow: a few faded offset copies in the secondary accent, then the crisp glyph
    for i in 1..=3 {
        let spread = i as f32 * 1.6;
        let a = 0.10 / i as f32;
        draw_text(title, tx - spread, y - spread, 40.0, Color::new(UI_ACCENT2.r, UI_ACCENT2.g, UI_ACCENT2.b, a));
        draw_text(title, tx + spread, y + spread, 40.0, Color::new(UI_ACCENT.r, UI_ACCENT.g, UI_ACCENT.b, a));
    }
    draw_text(title, tx + 2.0, y + 2.0, 40.0, BLACK);
    draw_text(title, tx, y, 40.0, UI_ACCENT);

    if let Some(sub) = subtitle {
        let s = measure_text(sub, None, 22, 1.0);
        draw_text(sub, center_x - s.width * 0.5, y + 32.0, 22.0, UI_TEXT_DIM);
    }
}

fn ui_label(x: f32, y: f32, txt: &str) {
    draw_text(txt, x, y, 20.0, UI_TEXT_DIM);
}

fn caret_visible() -> bool {
    ((get_time() * 1000.0) as i64 / 500) % 2 == 0
}

/// Defer state changes until after the frame to satisfy the borrow checker.
enum Transition {
    None,
    ToMenu,
    ToError(String),
    ToWaiting {
        net: NetClient,
        my: u32,
    },
    ToPlaying {
        net: NetClient,
        my: u32,
        accept: JoinAccept,
    },
    ToWinner {
        net: NetClient,
        standings: Vec<ScoreEntry>,
    },
}

/// Client high-level states.
enum ClientState {
    Menu,
    WaitingRoom(LobbyUi),
    Playing(PlayingUi),
    Winner(WinnerUi),
    Error(String),
}

struct MenuFields {
    server_addr: String,
    username: String,
    active: usize,
    status: String,
    suppress_input_frames: u8,
    skin: PlayerSkin,
}

impl Default for MenuFields {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1:45000".to_string(),
            username: "Player".to_string(),
            active: 0,
            status: "Enter server & name, then press Enter".into(),
            suppress_input_frames: 0,
            skin: PlayerSkin::Ninja,
        }
    }
}

struct LobbyUi {
    net: Option<NetClient>,
    my_id: u32,
    time_ms_left: u64,
    votes: [u32; 4],
    players: Vec<String>,
    selected: usize, // 0..=3
    last_update: std::time::Instant,
}

struct PlayingUi {
    net: Option<NetClient>,
    scene: Scene,
    my_id: u32,
    paused: bool,
    show_settings: bool,
    chat_open: bool,
    chat_input: String,
    chat_history: Vec<(String, String)>, // (player_name, message)
}

struct WinnerUi {
    net: Option<NetClient>,
    standings: Vec<ScoreEntry>,
    username: String,
    skin: PlayerSkin,
}

#[inline]
fn mouse_over(r: Rect) -> bool {
    let (mx, my) = mouse_position();
    r.contains(vec2(mx, my))
}

#[derive(Clone, Copy)]
enum ButtonKind {
    Primary,
    Ghost,
    Danger,
}

fn ui_button(r: Rect, label: &str, kind: ButtonKind) -> bool {
    let hovered = mouse_over(r);
    let (bg, border, txt, glow) = match kind {
        ButtonKind::Primary => {
            let base = UI_ACCENT;
            let bg = if hovered {
                Color::new(base.r, base.g, base.b, 0.95)
            } else {
                Color::new(base.r, base.g, base.b, 0.85)
            };
            (bg, WHITE, BLACK, base)
        }
        ButtonKind::Ghost => {
            let bg = if hovered {
                col(40, 44, 60, 200)
            } else {
                col(30, 32, 46, 150)
            };
            (bg, UI_BORDER, UI_TEXT, UI_BORDER)
        }
        ButtonKind::Danger => {
            let base = UI_DANGER;
            let bg = if hovered {
                Color::new(base.r, base.g, base.b, 0.95)
            } else {
                Color::new(base.r, base.g, base.b, 0.85)
            };
            (bg, WHITE, WHITE, base)
        }
    };

    draw_shadow(r, 4.0);
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    if hovered {
        draw_neon_glow(r, glow, 3);
    }
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, border);

    let t = measure_text(label, None, 22, 1.0);
    draw_text(
        label,
        r.x + (r.w - t.width) * 0.5,
        r.y + (r.h + 22.0) * 0.5 - 6.0,
        22.0,
        txt,
    );

    hovered && is_mouse_button_pressed(MouseButton::Left)
}

fn skin_display_name(skin: PlayerSkin) -> &'static str {
    match skin {
        PlayerSkin::Ninja => "Ninja",
        PlayerSkin::Alien => "Alien",
        PlayerSkin::Robot => "Robot",
        PlayerSkin::Pirate => "Pirate",
    }
}

/// Small flat-shaded preview icon of a skin, used on the menu's skin picker cards.
fn draw_skin_icon(cx: f32, top: f32, w: f32, h: f32, skin: PlayerSkin) {
    match skin {
        PlayerSkin::Ninja => {
            let outfit = col(15, 15, 20, 255);
            let mask = col(20, 20, 28, 255);
            let accent = col(200, 30, 30, 255);

            draw_rectangle(cx - w * 0.5, top, w, h, outfit);
            draw_rectangle_lines(cx - w * 0.5, top, w, h, 1.5, col(0, 0, 0, 160));

            let mask_top = top + h * 0.08;
            let mask_h = h * 0.22;
            draw_rectangle(cx - w * 0.5, mask_top, w, mask_h, mask);
            draw_rectangle_lines(cx - w * 0.5, mask_top, w, mask_h, 1.0, accent);

            let eye_y = mask_top + mask_h * 0.5;
            let eye_r = w * 0.12;
            draw_circle(cx - w * 0.20, eye_y, eye_r, col(220, 220, 220, 255));
            draw_circle(cx + w * 0.20, eye_y, eye_r, col(220, 220, 220, 255));
            draw_circle(cx - w * 0.20, eye_y, eye_r * 0.5, col(20, 20, 20, 255));
            draw_circle(cx + w * 0.20, eye_y, eye_r * 0.5, col(20, 20, 20, 255));

            let chest_accent_h = h * 0.08;
            draw_rectangle(cx - w * 0.15, top + h * 0.35, w * 0.30, chest_accent_h, accent);
        }
        PlayerSkin::Alien => {
            let skin = col(80, 200, 100, 255);
            let dark_skin = col(50, 140, 70, 255);

            draw_rectangle(cx - w * 0.5, top, w, h, skin);
            draw_ellipse(cx, top, w * 0.5, h * 0.07, 0.0, skin);
            draw_rectangle_lines(cx - w * 0.5, top, w, h, 1.5, col(0, 0, 0, 160));

            let head_center_y = top + h * 0.30;
            let eye_h = h * 0.18;
            let eye_w = w * 0.20;
            draw_ellipse(cx - w * 0.22, head_center_y, eye_w, eye_h, 0.0, col(255, 255, 255, 255));
            draw_ellipse(cx + w * 0.22, head_center_y, eye_w, eye_h, 0.0, col(255, 255, 255, 255));
            draw_circle(cx - w * 0.22, head_center_y, eye_w * 0.4, col(30, 30, 30, 255));
            draw_circle(cx + w * 0.22, head_center_y, eye_w * 0.4, col(30, 30, 30, 255));

            let stripe_y = top + h * 0.55;
            draw_rectangle(cx - w * 0.5, stripe_y, w, h * 0.06, dark_skin);
        }
        PlayerSkin::Robot => {
            let metal = col(120, 125, 135, 255);
            let panel = col(60, 80, 100, 255);
            let accent = col(200, 200, 200, 255);

            draw_rectangle(cx - w * 0.5, top, w, h, metal);
            draw_rectangle_lines(cx - w * 0.5, top, w, h, 1.5, col(0, 0, 0, 160));

            let panel_top = top + h * 0.12;
            let panel_h = h * 0.25;
            draw_rectangle(cx - w * 0.42, panel_top, w * 0.84, panel_h, panel);
            draw_rectangle_lines(cx - w * 0.42, panel_top, w * 0.84, panel_h, 1.0, accent);

            let eye_y = panel_top + panel_h * 0.5;
            let eye_s = w * 0.11;
            draw_rectangle(cx - w * 0.20 - eye_s * 0.5, eye_y - eye_s * 0.5, eye_s, eye_s, col(255, 100, 100, 255));
            draw_rectangle(cx + w * 0.20 - eye_s * 0.5, eye_y - eye_s * 0.5, eye_s, eye_s, col(255, 100, 100, 255));

            let chest_h = h * 0.10;
            draw_rectangle(cx - w * 0.38, top + h * 0.42, w * 0.76, chest_h, panel);
            draw_rectangle_lines(cx - w * 0.38, top + h * 0.42, w * 0.76, chest_h, 1.0, accent);
        }
        PlayerSkin::Pirate => {
            let coat = col(139, 69, 19, 255);
            let shirt = col(220, 200, 160, 255);
            let hat = col(40, 40, 50, 255);

            draw_rectangle(cx - w * 0.5, top, w, h, coat);
            draw_rectangle_lines(cx - w * 0.5, top, w, h, 1.5, col(0, 0, 0, 160));

            let face_y = top + h * 0.12;
            let face_h = h * 0.18;
            draw_rectangle(cx - w * 0.5, face_y, w, face_h, shirt);

            let hat_h = h * 0.08;
            draw_rectangle(cx - w * 0.5, top, w, hat_h, hat);
            draw_line(cx - w * 0.48, top + hat_h, cx + w * 0.48, top + hat_h, 2.0, col(255, 215, 0, 255));

            let eye_y = face_y + face_h * 0.5;
            draw_circle(cx - w * 0.15, eye_y, w * 0.08, col(220, 220, 220, 255));
            draw_circle(cx - w * 0.15, eye_y, w * 0.04, col(30, 30, 30, 255));

            let patch_r = w * 0.10;
            draw_circle(cx + w * 0.15, eye_y, patch_r, col(30, 30, 30, 255));
            draw_circle_lines(cx + w * 0.15, eye_y, patch_r, 1.5, col(139, 69, 19, 255));

            let hook_x = cx + w * 0.35;
            draw_line(hook_x, top + h * 0.45, hook_x, top + h * 0.65, 2.0, col(200, 200, 100, 255));
            draw_circle(hook_x, top + h * 0.65, w * 0.05, col(200, 200, 100, 255));
        }
    }
}

/// Small maze-like preview icon for a map vote card: a deterministic speckled grid
/// tinted by difficulty (green/cyan/red), or a dice-and-question-mark motif for Random.
fn draw_map_icon(rect: Rect, difficulty: usize) {
    let (wall_col, path_col, density, seed): (Color, Color, f32, i32) = match difficulty {
        0 => (col(70, 220, 130, 255), col(14, 34, 26, 255), 0.26, 11),
        1 => (col(60, 175, 225, 255), col(14, 26, 38, 255), 0.40, 23),
        2 => (col(235, 90, 70, 255), col(38, 16, 16, 255), 0.56, 37),
        _ => (col(200, 90, 235, 255), col(28, 16, 38, 255), 0.0, 51),
    };

    draw_rectangle(rect.x, rect.y, rect.w, rect.h, path_col);

    if difficulty == 3 {
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.5, wall_col);

        let pip_r = rect.h * 0.06;
        let pips: [(f32, f32); 5] = [(-0.30, -0.30), (0.30, -0.30), (-0.30, 0.30), (0.30, 0.30), (0.0, 0.0)];
        for (dx, dy) in pips {
            draw_circle(
                rect.x + rect.w * (0.5 + dx * 0.55),
                rect.y + rect.h * (0.5 + dy * 0.55),
                pip_r,
                Color::new(wall_col.r, wall_col.g, wall_col.b, 0.35),
            );
        }

        let fs = rect.h * 0.62;
        let qt = measure_text("?", None, fs as u16, 1.0);
        draw_text(
            "?",
            rect.x + rect.w * 0.5 - qt.width * 0.5,
            rect.y + rect.h * 0.5 + fs * 0.32,
            fs,
            wall_col,
        );
        return;
    }

    let cols = 10i32;
    let rows = 3i32;
    let cell_w = rect.w / cols as f32;
    let cell_h = rect.h / rows as f32;
    for gy in 0..rows {
        for gx in 0..cols {
            let hash = ((gx * 928371 + gy * 121933 + seed * 6151) & 0x7fffffff) % 100;
            if (hash as f32 / 100.0) < density {
                let x = rect.x + gx as f32 * cell_w;
                let y = rect.y + gy as f32 * cell_h;
                draw_rectangle(x + 0.5, y + 0.5, cell_w - 1.0, cell_h - 1.0, wall_col);
            }
        }
    }
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.5, wall_col);
}

/// Selectable card showing a skin preview icon plus its name (used in the menu skin picker).
fn skin_card_button(r: Rect, skin: PlayerSkin, label: &str, selected: bool) -> bool {
    let hovered = mouse_over(r);
    let bg = if selected {
        Color::new(UI_ACCENT.r, UI_ACCENT.g, UI_ACCENT.b, if hovered { 0.32 } else { 0.20 })
    } else if hovered {
        col(40, 44, 52, 180)
    } else {
        col(35, 38, 46, 150)
    };
    let border = if selected { UI_ACCENT } else { UI_BORDER };

    draw_shadow(r, 4.0);
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    if selected || hovered {
        draw_neon_glow(r, border, if selected { 4 } else { 2 });
    }
    draw_rectangle_lines(r.x, r.y, r.w, r.h, if selected { 3.0 } else { 2.0 }, border);

    let label_h = 26.0;
    let icon_pad = 10.0;
    let icon_top = r.y + icon_pad;
    let icon_h = (r.h - label_h - icon_pad * 2.0).max(10.0);
    let icon_w = (icon_h * 0.55).min(r.w - icon_pad * 2.0);
    draw_skin_icon(r.x + r.w * 0.5, icon_top, icon_w, icon_h, skin);

    let t = measure_text(label, None, 20, 1.0);
    let txt_col = if selected { UI_ACCENT } else { UI_TEXT };
    draw_text(label, r.x + (r.w - t.width) * 0.5, r.y + r.h - 10.0, 20.0, txt_col);

    hovered && is_mouse_button_pressed(MouseButton::Left)
}

#[derive(Clone, Copy)]
enum FieldKind {
    Server,
    Username,
}

fn text_input_filtered(field: &mut String, active: bool, kind: FieldKind) {
    if !active {
        return;
    }
    while let Some(c) = get_char_pressed() {
        match c {
            '\u{8}' => {
                field.pop(); // backspace
            }
            '\n' | '\r' | '\t' => {}
            _ => {
                let allow = match kind {
                    FieldKind::Server => {
                        c.is_ascii_alphanumeric() || c == '.' || c == ':' || c == '-'
                    }
                    FieldKind::Username => c.is_ascii_alphanumeric() || c == '_',
                };
                if allow {
                    field.push(c);
                }
            }
        }
    }
}

fn sanitize_server(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == ':' || *c == '-')
        .collect()
}

fn sanitize_username(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        }
    }
    out
}

/// NEW: shared connect logic used by Enter key and Connect button
fn try_connect(menu: &mut MenuFields) -> Option<Transition> {
    menu.server_addr = sanitize_server(&menu.server_addr);
    menu.username = sanitize_username(&menu.username);

    if menu.server_addr.is_empty() {
        menu.status = "Server address is empty".into();
        return None;
    }
    if menu.username.len() < 3 || menu.username.len() > 16 {
        menu.status = "Username must be 3–16 chars [A-Z a-z 0-9 _]".into();
        return None;
    }
    let srv: SocketAddr = match menu.server_addr.parse() {
        Ok(a) => a,
        Err(_) => {
            menu.status = "Invalid server (use ip:port)".into();
            return None;
        }
    };
    let local = "0.0.0.0:0";
    match NetClient::connect(local, srv) {
        Ok(net) => {
            let name = menu.username.clone();
            if let Err(e) = net.join(name, menu.skin) {
                menu.status = format!("Join send failed: {e}");
                None
            } else {
                Some(Transition::ToWaiting { net, my: 0 })
            }
        }
        Err(e) => {
            menu.status = format!("Socket error: {e}");
            None
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut state = ClientState::Menu;
    let mut menu = MenuFields::default();

    loop {
        // Background: deep-space base + neon corner glows + vignette
        let w = screen_width();
        let h = screen_height();
        draw_rectangle(0.0, 0.0, w, h, col(5, 3, 11, 255));
        draw_rectangle(0.0, 0.0, w * 0.55, h * 0.55, Color::new(UI_ACCENT.r, UI_ACCENT.g, UI_ACCENT.b, 0.05));
        draw_rectangle(w * 0.45, h * 0.45, w * 0.55, h * 0.55, Color::new(UI_ACCENT2.r, UI_ACCENT2.g, UI_ACCENT2.b, 0.05));
        for i in 0..8 {
            let a = (i as f32) / 8.0;
            let c = lerp(UI_BG, UI_CARD, a * 0.5 + 0.2);
            let pad = (i as f32) * 22.0;
            draw_rectangle(
                pad,
                pad,
                w - pad * 2.0,
                h - pad * 2.0,
                Color::new(c.r, c.g, c.b, 0.04),
            );
        }
        // faint CRT scanlines for a retro-cyberpunk texture
        let mut sl_y = 0.0;
        while sl_y < h {
            draw_line(0.0, sl_y, w, sl_y, 1.0, col(0, 0, 0, 16));
            sl_y += 3.0;
        }

        let mut transition = Transition::None;

        let input_locked = match &state {
            ClientState::Menu => menu.suppress_input_frames > 0,
            _ => false,
        };

        match &mut state {
            // ---------------- MENU ----------------
            ClientState::Menu => {
                // Main panel: two-column "character select" layout
                let panel_w = (w * 0.66).clamp(920.0, 1200.0);
                let panel_h = 480.0;
                let panel = Rect::new((w - panel_w) * 0.5, (h - panel_h) * 0.42, panel_w, panel_h);
                draw_panel(panel);

                ui_title(
                    w * 0.5,
                    panel.y - 64.0,
                    "MAZE WARS",
                    Some("Choose your warrior - Battle in the darkness"),
                );

                // Drain stray input when returning
                if menu.suppress_input_frames > 0 {
                    let mut drained = 0;
                    while get_char_pressed().is_some() {
                        drained += 1;
                        if drained > 128 {
                            break;
                        }
                    }
                    menu.suppress_input_frames -= 1;
                }

                let content_pad = 32.0;
                let col_gap = 28.0;
                let usable_w = panel.w - content_pad * 2.0 - col_gap;
                let left_w = usable_w * 0.52;
                let right_w = usable_w - left_w;
                let left_x = panel.x + content_pad;
                let right_x = left_x + left_w + col_gap;
                let top_y = panel.y + 30.0;

                // ---- LEFT COLUMN: connection details ----
                ui_label(left_x, top_y + 20.0, "Server (ip:port)");
                let server_rect = Rect::new(left_x, top_y + 32.0, left_w, 44.0);
                draw_card(server_rect, 2.0);
                let active0 = menu.active == 0;
                if active0 {
                    draw_rectangle_lines(
                        server_rect.x,
                        server_rect.y,
                        server_rect.w,
                        server_rect.h,
                        2.0,
                        UI_ACCENT,
                    );
                }
                let sv = format!(
                    "{}{}",
                    menu.server_addr,
                    if active0 && caret_visible() { "|" } else { "" }
                );
                draw_text(
                    &sv,
                    server_rect.x + 12.0,
                    server_rect.y + 30.0,
                    22.0,
                    UI_TEXT,
                );

                ui_label(left_x, top_y + 92.0, "Username");
                let user_rect = Rect::new(left_x, top_y + 104.0, left_w, 44.0);
                draw_card(user_rect, 2.0);
                let active1 = menu.active == 1;
                if active1 {
                    draw_rectangle_lines(
                        user_rect.x,
                        user_rect.y,
                        user_rect.w,
                        user_rect.h,
                        2.0,
                        UI_ACCENT,
                    );
                }
                let uv = format!(
                    "{}{}",
                    menu.username,
                    if active1 && caret_visible() { "|" } else { "" }
                );
                draw_text(&uv, user_rect.x + 12.0, user_rect.y + 30.0, 22.0, UI_TEXT);

                // Status feedback lives under the fields it relates to
                let st = format!("Status: {}", menu.status);
                draw_text(&st, left_x, top_y + 178.0, 18.0, UI_TEXT_DIM);

                // vertical divider between the two columns
                let divider_x = left_x + left_w + col_gap * 0.5;
                draw_line(divider_x, top_y - 4.0, divider_x, top_y + 284.0, 1.0, col(70, 90, 120, 90));

                // ---- RIGHT COLUMN: fighter select ----
                ui_label(right_x, top_y + 4.0, "Choose Your Fighter");

                let skins = [
                    (PlayerSkin::Ninja, "Ninja"),
                    (PlayerSkin::Alien, "Alien"),
                    (PlayerSkin::Robot, "Robot"),
                    (PlayerSkin::Pirate, "Pirate"),
                ];
                let (_, selected_label) = skins.iter().find(|(k, _)| *k == menu.skin).unwrap();

                // big portrait card
                let preview_rect = Rect::new(right_x, top_y + 16.0, right_w, 150.0);
                draw_card(preview_rect, 2.0);
                draw_neon_glow(preview_rect, UI_ACCENT, 2);
                let icon_h = preview_rect.h - 46.0;
                let icon_w = (icon_h * 0.55).min(preview_rect.w * 0.4);
                draw_skin_icon(preview_rect.x + preview_rect.w * 0.5, preview_rect.y + 14.0, icon_w, icon_h, menu.skin);
                let lt = measure_text(selected_label, None, 24, 1.0);
                draw_text(
                    selected_label,
                    preview_rect.x + (preview_rect.w - lt.width) * 0.5,
                    preview_rect.y + preview_rect.h - 14.0,
                    24.0,
                    UI_ACCENT,
                );

                // thumbnail strip to pick a fighter
                let thumb_gap = 14.0;
                let skin_count = skins.len() as f32;
                let thumb_w = (right_w - thumb_gap * (skin_count - 1.0)) / skin_count;
                let thumb_h = 96.0;
                let thumb_y = preview_rect.y + preview_rect.h + thumb_gap;
                for (i, (kind, label)) in skins.iter().enumerate() {
                    let tx = right_x + (thumb_w + thumb_gap) * i as f32;
                    let selected = menu.skin == *kind;
                    if skin_card_button(Rect::new(tx, thumb_y, thumb_w, thumb_h), *kind, label, selected) {
                        menu.skin = *kind;
                    }
                }

                // ---- FOOTER: primary actions ----
                let bw = 180.0;
                let bh = 46.0;
                let gap = 16.0;
                let total = bw * 2.0 + gap;
                let bx = (w - total) * 0.5;
                let by = panel.y + panel_h - 62.0;

                if ui_button(Rect::new(bx, by, bw, bh), "Connect", ButtonKind::Primary) {
                    if let Some(t) = try_connect(&mut menu) {
                        transition = t;
                    }
                }
                if ui_button(
                    Rect::new(bx + bw + gap, by, bw, bh),
                    "Quit",
                    ButtonKind::Ghost,
                ) {
                    break;
                }

                // Keyboard focus switch (blocked while input_locked)
                if !input_locked && is_key_pressed(KeyCode::Tab) {
                    menu.active = (menu.active + 1) % 2;
                }

                // Text input (blocked while input_locked)
                if !input_locked {
                    if menu.active == 0 {
                        text_input_filtered(&mut menu.server_addr, true, FieldKind::Server);
                    } else {
                        text_input_filtered(&mut menu.username, true, FieldKind::Username);
                    }
                }

                // Connect via keyboard Enter (blocked while input_locked)
                if !input_locked && is_key_pressed(KeyCode::Enter) {
                    if let Some(t) = try_connect(&mut menu) {
                        transition = t;
                    }
                }
            }

            // ---------------- LOBBY ----------------
            ClientState::WaitingRoom(lobby) => {
                if let Some(net) = lobby.net.as_mut() {
                    net.keepalive_tick();
                }

                // Read server messages
                let mut leave_loop = false;
                loop {
                    let msg_opt = {
                        if let Some(net) = lobby.net.as_mut() {
                            net.poll()
                        } else {
                            None
                        }
                    };
                    let Some(msg) = msg_opt else { break };
                    match msg {
                        ServerToClient::JoinOk(accept) => {
                            lobby.my_id = accept.player_id;
                            lobby.time_ms_left = accept.lobby_ms_left;
                            lobby.last_update = std::time::Instant::now();
                        }
                        ServerToClient::LobbyState(st) => {
                            lobby.time_ms_left = st.time_ms_left;
                            lobby.votes = st.votes;
                            lobby.players = st.players;
                            lobby.last_update = std::time::Instant::now();
                        }
                        ServerToClient::LevelChanged(accept) => {
                            if let Some(net) = lobby.net.take() {
                                transition = Transition::ToPlaying {
                                    net,
                                    my: lobby.my_id,
                                    accept,
                                };
                                leave_loop = true;
                            }
                        }
                        ServerToClient::MatchOver { standings } => {
                            if let Some(net) = lobby.net.take() {
                                transition = Transition::ToWinner { net, standings };
                                leave_loop = true;
                            }
                        }
                        ServerToClient::JoinRejected(r) => {
                            transition =
                                Transition::ToError(format!("Join rejected: {}", r.reason));
                            leave_loop = true;
                        }
                        ServerToClient::ServerFull => {
                            transition = Transition::ToError("Server full".into());
                            leave_loop = true;
                        }
                        ServerToClient::Snapshot(_)
                        | ServerToClient::HitConfirm { .. }
                        | ServerToClient::Pong(_)
                        | ServerToClient::ChatMessage(_) => {}
                    }
                    if leave_loop {
                        break;
                    }
                }

                // Layout — panel height is derived from content, so there's no dead space
                // below the players list (it shrinks/grows to fit however many joined).
                // A "Match Info" side panel fills the leftover width to the right.
                let panel_w = (w * 0.66).clamp(760.0, 1080.0);
                let side_w = (w * 0.20).clamp(240.0, 320.0);
                let group_gap = 24.0;
                let card_h = 150.0;
                let players_shown = lobby.players.len().max(1);
                let plist_h = 76.0 + (players_shown as f32 - 1.0) * 22.0;
                let panel_h = 26.0   // top padding above the vote cards
                    + card_h          // vote cards row
                    + 26.0            // gap to the waiting/countdown line
                    + 26.0            // gap to the players card
                    + plist_h         // players card, sized to its rows
                    + 34.0            // gap to the footer hint
                    + 16.0;           // bottom padding
                let total_w = panel_w + group_gap + side_w;
                let group_x = ((w - total_w) * 0.5).max(16.0);
                let panel = Rect::new(group_x, (h - panel_h) * 0.48, panel_w, panel_h);
                draw_panel(panel);
                ui_title(
                    group_x + total_w * 0.5,
                    panel.y - 68.0,
                    "VOTE FOR MAP",
                    Some("First to start when players more than 2"),
                );

                // ---- Match Info side panel ----
                let side_panel = Rect::new(panel.x + panel.w + group_gap, panel.y, side_w, panel_h);
                draw_panel(side_panel);
                let sx = side_panel.x + 20.0;
                let mut sy = side_panel.y + 30.0;
                draw_text("Match Info", sx, sy, 22.0, UI_ACCENT);
                sy += 10.0;
                draw_line(side_panel.x + 16.0, sy, side_panel.x + side_panel.w - 16.0, sy, 1.0, UI_BORDER);
                sy += 22.0;

                // your chosen fighter, reusing the same icon art as the menu
                let fighter_icon_h = 46.0;
                let fighter_icon_w = fighter_icon_h * 0.55;
                draw_skin_icon(sx + fighter_icon_w * 0.5, sy, fighter_icon_w, fighter_icon_h, menu.skin);
                draw_text("Your Fighter", sx + fighter_icon_w + 16.0, sy + fighter_icon_h * 0.4, 14.0, UI_TEXT_DIM);
                draw_text(
                    skin_display_name(menu.skin),
                    sx + fighter_icon_w + 16.0,
                    sy + fighter_icon_h * 0.4 + 22.0,
                    19.0,
                    UI_TEXT,
                );
                sy += fighter_icon_h + 16.0;

                draw_line(side_panel.x + 16.0, sy, side_panel.x + side_panel.w - 16.0, sy, 1.0, col(60, 70, 90, 90));
                sy += 22.0;

                draw_text("Rules", sx, sy, 17.0, UI_ACCENT2);
                sy += 20.0;
                for line in ["Mode: Deathmatch", "Needs 2+ players", "Highest score wins"] {
                    draw_text(line, sx, sy, 15.0, UI_TEXT_DIM);
                    sy += 18.0;
                }
                sy += 10.0;

                draw_text("Controls", sx, sy, 17.0, UI_ACCENT2);
                sy += 20.0;
                for line in ["[WASD] Move", "[Mouse] Look", "[LMB/Space] Fire", "[T] Chat  [Tab] Scoreboard"] {
                    draw_text(line, sx, sy, 15.0, UI_TEXT_DIM);
                    sy += 18.0;
                }

                // Vote cards
                let labels = ["Map 1 (Easy)", "Map 2 (Normal)", "Map 3 (Hard)", "Random"];
                let cards_w = panel.w - 48.0;
                let card_w = (cards_w - 3.0 * 16.0) / 4.0;
                let x0 = panel.x + 24.0;
                let y0 = panel.y + 26.0;

                for i in 0..4 {
                    let x = x0 + (card_w + 16.0) * i as f32;
                    let r = Rect::new(x, y0, card_w, card_h);
                    draw_card(r, 2.0);

                    // thumbnail: mini maze preview, tinted by difficulty
                    let thumb = Rect::new(r.x, r.y, r.w, 68.0);
                    draw_map_icon(thumb, i);

                    // title
                    let lbl = labels[i];
                    let tt = measure_text(lbl, None, 20, 1.0);
                    draw_text(lbl, r.x + (r.w - tt.width) * 0.5, r.y + 92.0, 20.0, UI_TEXT);

                    // vote bar
                    let v = lobby.votes[i] as f32;
                    let vmax = (lobby.votes.iter().copied().max().unwrap_or(1)) as f32;
                    let ratio = if vmax <= 0.0 {
                        0.0
                    } else {
                        (v / vmax).clamp(0.0, 1.0)
                    };
                    let bar = Rect::new(r.x + 14.0, r.y + r.h - 38.0, r.w - 28.0, 18.0);
                    draw_rectangle(bar.x, bar.y, bar.w, bar.h, col(28, 32, 40, 200));
                    draw_rectangle(
                        bar.x,
                        bar.y,
                        bar.w * ratio,
                        bar.h,
                        Color::new(UI_ACCENT.r, UI_ACCENT.g, UI_ACCENT.b, 0.85),
                    );
                    draw_rectangle_lines(bar.x, bar.y, bar.w, bar.h, 2.0, UI_BORDER);

                    // numeric vote badge
                    let count = lobby.votes[i];
                    let badge_cx = r.x + r.w - 16.0;
                    let badge_cy = r.y + 16.0;
                    draw_circle(badge_cx, badge_cy, 12.0, col(255, 255, 193, 7));
                    draw_circle_lines(badge_cx, badge_cy, 12.0, 1.6, UI_BORDER);
                    let s = count.to_string();
                    let m = measure_text(&s, None, 16, 1.0);
                    draw_text(
                        &s,
                        badge_cx - m.width / 2.0,
                        badge_cy + m.offset_y / 2.0 + 1.0,
                        16.0,
                        BLACK,
                    );

                    // selection ring
                    if i == lobby.selected {
                        draw_rectangle_lines(
                            r.x - 2.0,
                            r.y - 2.0,
                            r.w + 4.0,
                            r.h + 4.0,
                            3.0,
                            UI_ACCENT,
                        );
                    }
                }

                // Countdown / players
                let elapsed_ms = lobby.last_update.elapsed().as_millis() as u64;
                let ms_left = lobby.time_ms_left.saturating_sub(elapsed_ms);
                let waiting_for_players = lobby.players.len() < 2;

                let info_y = y0 + card_h + 26.0;
                if waiting_for_players {
                    draw_text(
                        "Waiting for players to join (need at least 2)",
                        panel.x + 24.0,
                        info_y,
                        22.0,
                        UI_TEXT_DIM,
                    );
                } else {
                    let secs = ((ms_left as f32) / 1000.0).ceil().max(0.0) as i32;
                    let cd = format!("Match starts in: {}s", secs);
                    draw_text(&cd, panel.x + 24.0, info_y, 22.0, UI_OK);
                }

                // Players list card — height already fitted to `players_shown` rows above
                let plist = Rect::new(panel.x + 24.0, info_y + 26.0, panel.w - 48.0, plist_h);
                draw_card(plist, 2.0);
                let mut yy = plist.y + 26.0;
                draw_text("Players", plist.x + 16.0, yy, 22.0, UI_TEXT);
                yy += 10.0;
                draw_line(
                    plist.x + 12.0,
                    yy,
                    plist.x + plist.w - 12.0,
                    yy,
                    1.0,
                    UI_BORDER,
                );
                yy += 22.0;
                if lobby.players.is_empty() {
                    draw_text("Waiting for players...", plist.x + 18.0, yy, 20.0, UI_TEXT_DIM);
                } else {
                    for name in &lobby.players {
                        draw_text(name, plist.x + 18.0, yy, 20.0, UI_TEXT_DIM);
                        yy += 22.0;
                    }
                }

                // Footer hint + controls
                draw_text(
                    "[Right Arrow/Left Arrow] select map   [Enter] vote   [Esc] Main Menu",
                    panel.x + 24.0,
                    panel.y + panel.h - 16.0,
                    18.0,
                    UI_TEXT_DIM,
                );

                // Inputs
                if is_key_pressed(KeyCode::Left) && lobby.selected > 0 {
                    lobby.selected -= 1;
                }
                if is_key_pressed(KeyCode::Right) && lobby.selected < 3 {
                    lobby.selected += 1;
                }
                if is_key_pressed(KeyCode::Enter) {
                    let choice = match lobby.selected {
                        0 => MapChoice::Map1,
                        1 => MapChoice::Map2,
                        2 => MapChoice::Map3,
                        _ => MapChoice::Random,
                    };
                    if let Some(net) = lobby.net.as_ref() {
                        net.vote(choice);
                    }
                }
                if is_key_pressed(KeyCode::Escape) {
                    transition = Transition::ToMenu;
                }
            }

            // ---------------- PLAYING ----------------
            ClientState::Playing(ui) => {
                // Chat toggle (T)
                if is_key_pressed(KeyCode::T) && !ui.chat_open && !ui.paused {
                    ui.chat_open = true;
                    ui.chat_input.clear();
                    set_cursor_grab(false);
                    show_mouse(true);
                }

                // Pause toggling
                if is_key_pressed(KeyCode::Escape) {
                    if ui.chat_open {
                        ui.chat_open = false;
                        set_cursor_grab(true);
                        show_mouse(false);
                    } else if !ui.paused {
                        ui.paused = true;
                        ui.show_settings = false;
                        set_cursor_grab(false);
                        show_mouse(true);
                    } else if ui.show_settings {
                        ui.show_settings = false;
                    } else {
                        ui.paused = false;
                        set_cursor_grab(true);
                        show_mouse(false);
                    }
                }

                if let Some(net) = ui.net.as_mut() {
                    net.keepalive_tick();
                }

                // Poll messages
                let mut leave_loop = false;
                loop {
                    let msg_opt = { ui.net.as_mut().and_then(|n| n.poll()) };
                    let Some(msg) = msg_opt else { break };
                    match msg {
                        ServerToClient::Snapshot(snap) => ui.scene.apply_snapshot(&snap),
                        ServerToClient::HitConfirm { .. } => ui.scene.on_hit_confirm(),
                        ServerToClient::MatchOver { standings } => {
                            if let Some(net) = ui.net.take() {
                                transition = Transition::ToWinner { net, standings };
                                leave_loop = true;
                            }
                        }
                        ServerToClient::LevelChanged(accept) => {
                            ui.scene.apply_join(&accept, ui.my_id)
                        }
                        ServerToClient::ChatMessage(chat) => {
                            ui.chat_history.push((chat.player_name, chat.message));
                            if ui.chat_history.len() > 50 {
                                ui.chat_history.remove(0);
                            }
                        }
                        ServerToClient::Pong(_) => {}
                        ServerToClient::JoinOk(_)
                        | ServerToClient::LobbyState(_)
                        | ServerToClient::JoinRejected(_)
                        | ServerToClient::ServerFull => {}
                    }
                    if leave_loop {
                        break;
                    }
                }

                // Chat input
                if ui.chat_open {
                    while let Some(c) = get_char_pressed() {
                        match c {
                            '\u{8}' => {
                                ui.chat_input.pop();
                            }
                            '\n' | '\r' => {
                                if !ui.chat_input.trim().is_empty() {
                                    if let Some(net) = ui.net.as_ref() {
                                        net.send_chat(ui.chat_input.clone());
                                    }
                                }
                                ui.chat_open = false;
                                ui.chat_input.clear();
                                set_cursor_grab(true);
                                show_mouse(false);
                            }
                            _ if ui.chat_input.len() < 100 => {
                                ui.chat_input.push(c);
                            }
                            _ => {}
                        }
                    }
                }

                // Send input if active
                if !ui.paused && !ui.chat_open {
                    if let Some(net) = ui.net.as_mut() {
                        let input = ui.scene.build_input();
                        net.send_input(&input);
                        if ui.scene.fire_just() {
                            if let Some((px, py, yaw)) = ui.scene.my_pose() {
                                let len = ui.scene.fire_raycast_length(px, py, yaw, 30.0);
                                ui.scene.spawn_shot_fx(yaw, len);
                            }
                        }
                        ui.scene.tick_fire_fx();
                    }
                }

                // Draw world/HUD
                ui.scene.draw();

                // Chat history (last 5)
                let mut cy = screen_height() - 140.0;
                for (name, msg) in ui.chat_history.iter().rev().take(5).rev() {
                    let text = format!("{}: {}", name, msg);
                    draw_text(&text, 12.0, cy, 18.0, col(255, 255, 255, 200));
                    cy += 20.0;
                }

                // Chat input box
                if ui.chat_open {
                    let box_h = 36.0;
                    let box_y = screen_height() - 46.0;
                    draw_shadow(Rect::new(10.0, box_y, screen_width() - 20.0, box_h), 3.0);
                    draw_rectangle(10.0, box_y, screen_width() - 20.0, box_h, UI_BG);
                    draw_rectangle_lines(10.0, box_y, screen_width() - 20.0, box_h, 2.0, UI_ACCENT);
                    let prompt = format!(
                        "Say: {}{}",
                        ui.chat_input,
                        if caret_visible() { "|" } else { "" }
                    );
                    draw_text(&prompt, 16.0, box_y + 24.0, 20.0, UI_TEXT);
                }

                if ui.paused {
                    draw_pause_overlay(ui, &mut transition);
                } else if !ui.chat_open {
                    draw_text(
                        "[W/S] move   [A/D] strafe   [Mouse] look   [LMB/Space] fire   [Tab] scoreboard   [T] chat",
                        12.0,
                        screen_height() - 12.0,
                        18.0,
                        UI_TEXT_DIM,
                    );
                }
            }

            // ---------------- WINNER ----------------
            ClientState::Winner(win) => {
                if let Some(net) = win.net.as_mut() {
                    net.keepalive_tick();
                }

                // Poll rejoin outcomes
                let mut leave_loop = false;
                loop {
                    let msg_opt = { win.net.as_mut().and_then(|n| n.poll()) };
                    let Some(msg) = msg_opt else { break };
                    match msg {
                        ServerToClient::JoinOk(accept) => {
                            if let Some(net) = win.net.take() {
                                transition = Transition::ToWaiting {
                                    net,
                                    my: accept.player_id,
                                };
                                leave_loop = true;
                            }
                        }
                        ServerToClient::JoinRejected(r) => {
                            transition =
                                Transition::ToError(format!("Rejoin failed: {}", r.reason));
                            leave_loop = true;
                        }
                        _ => {}
                    }
                    if leave_loop {
                        break;
                    }
                }

                // Layout
                let pw = (w * 0.80).clamp(820.0, 1400.0);
                let ph = (h * 0.62).clamp(420.0, 700.0);
                let rect = Rect::new((w - pw) * 0.5, (h - ph) * 0.48, pw, ph);
                draw_panel(rect);

                ui_title(w * 0.5, rect.y - 64.0, "MATCH OVER", Some("Great fight!"));

                // Winner banner (no emoji)
                if let Some(top) = win.standings.first() {
                    let banner = Rect::new(rect.x + 24.0, rect.y + 22.0, rect.w - 48.0, 76.0);
                    draw_card(banner, 2.0);
                    let title = format!(
                        "Winner: {}   •   Score {}   •   K/D {}/{}",
                        top.name, top.score, top.kills, top.deaths
                    );
                    let tt = measure_text(&title, None, 28, 1.0);
                    draw_text(
                        &title,
                        banner.x + (banner.w - tt.width) * 0.5,
                        banner.y + 50.0,
                        28.0,
                        UI_TEXT,
                    );
                }

                // Scoreboard table
                let table = Rect::new(rect.x + 24.0, rect.y + 106.0, rect.w - 48.0, rect.h - 180.0);
                draw_card(table, 2.0);
                let headers = ["#", "Player", "Kills", "Deaths", "Score"];
                let cols = [0.06, 0.22, 0.62, 0.76, 0.90];
                let mut y = table.y + 26.0;

                for (i, hname) in headers.iter().enumerate() {
                    draw_text(hname, table.x + table.w * cols[i], y, 20.0, UI_TEXT_DIM);
                }
                y += 8.0;
                draw_line(
                    table.x + 12.0,
                    y,
                    table.x + table.w - 12.0,
                    y,
                    1.0,
                    UI_BORDER,
                );
                y += 26.0;

                for (i, row) in win.standings.iter().enumerate() {
                    let name_col = if i == 0 { UI_ACCENT } else { UI_TEXT };
                    draw_text(
                        &format!("{}", i + 1),
                        table.x + table.w * cols[0],
                        y,
                        20.0,
                        UI_TEXT,
                    );
                    draw_text(&row.name, table.x + table.w * cols[1], y, 20.0, name_col);
                    draw_text(
                        &format!("{}", row.kills),
                        table.x + table.w * cols[2],
                        y,
                        20.0,
                        UI_TEXT,
                    );
                    draw_text(
                        &format!("{}", row.deaths),
                        table.x + table.w * cols[3],
                        y,
                        20.0,
                        UI_TEXT,
                    );
                    draw_text(
                        &format!("{}", row.score),
                        table.x + table.w * cols[4],
                        y,
                        20.0,
                        UI_TEXT,
                    );
                    y += 22.0;
                }

                // Buttons
                let bw = 220.0;
                let bh = 44.0;
                let gap = 24.0;
                let total = bw * 2.0 + gap;
                let bx = (w - total) * 0.5;
                let by = rect.y + rect.h - 60.0;

                if ui_button(
                    Rect::new(bx, by, bw, bh),
                    "Return to Lobby",
                    ButtonKind::Primary,
                ) {
                    if let Some(net) = win.net.as_ref() {
                        let _ = net.return_to_lobby(win.username.clone(), win.skin);
                    }
                }
                if ui_button(
                    Rect::new(bx + bw + gap, by, bw, bh),
                    "Main Menu",
                    ButtonKind::Ghost,
                ) {
                    transition = Transition::ToMenu;
                }

                draw_text(
                    "Press [Esc] for Main Menu",
                    12.0,
                    h - 12.0,
                    18.0,
                    UI_TEXT_DIM,
                );
                if is_key_pressed(KeyCode::Escape) {
                    transition = Transition::ToMenu;
                }
            }

            // ---------------- ERROR ----------------
            ClientState::Error(msg) => {
                let box_w = (w * 0.58).clamp(600.0, 960.0);
                let box_h = 220.0;
                let rect = Rect::new((w - box_w) * 0.5, (h - box_h) * 0.48, box_w, box_h);
                draw_panel(rect);
                ui_title(w * 0.5, rect.y - 58.0, "ERROR", None);
                draw_text(msg, rect.x + 24.0, rect.y + 84.0, 22.0, UI_TEXT);
                draw_text(
                    "[Esc] back to menu",
                    rect.x + 24.0,
                    rect.y + 120.0,
                    18.0,
                    UI_TEXT_DIM,
                );
                if is_key_pressed(KeyCode::Escape) {
                    transition = Transition::ToMenu;
                }
            }
        }

        // -------- apply staged transition --------
        match transition {
            Transition::None => {}
            Transition::ToMenu => {
                state = ClientState::Menu;
                set_cursor_grab(false);
                show_mouse(true);
                menu.suppress_input_frames = 12; // was 3
                let mut drained = 0;
                while let Some(_c) = get_char_pressed() {
                    drained += 1;
                    if drained > 512 {
                        break;
                    } // slightly higher just in case
                }
            }

            Transition::ToError(e) => {
                state = ClientState::Error(e);
            }
            Transition::ToWaiting { net, my } => {
                set_cursor_grab(false);
                show_mouse(true);
                state = ClientState::WaitingRoom(LobbyUi {
                    net: Some(net),
                    my_id: my,
                    time_ms_left: 0,
                    votes: [0; 4],
                    players: vec![],
                    selected: 0,
                    last_update: std::time::Instant::now(),
                });
            }
            Transition::ToPlaying { net, my, accept } => {
                set_cursor_grab(true);
                show_mouse(false);
                let mut scene = Scene::new(my, accept.width, accept.height, accept.tiles.clone());
                scene.apply_join(&accept, my);
                state = ClientState::Playing(PlayingUi {
                    net: Some(net),
                    scene,
                    my_id: my,
                    paused: false,
                    show_settings: false,
                    chat_open: false,
                    chat_input: String::new(),
                    chat_history: Vec::new(),
                });
            }
            Transition::ToWinner { net, standings } => {
                set_cursor_grab(false);
                show_mouse(true);
                state = ClientState::Winner(WinnerUi {
                    net: Some(net),
                    standings,
                    username: menu.username.clone(),
                    skin: menu.skin,
                });
            }
        }

        next_frame().await;
    }
}

// -------- Pause/Settings overlay drawing --------
fn draw_pause_overlay(ui: &mut PlayingUi, transition: &mut Transition) {
    let w = screen_width();
    let h = screen_height();

    // dim bg
    draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.62));

    let panel_w = (w * 0.44).clamp(520.0, 900.0);
    let panel_h = (h * 0.56).clamp(420.0, 640.0);
    let x = (w - panel_w) * 0.5;
    let y = (h - panel_h) * 0.5;
    let panel = Rect::new(x, y, panel_w, panel_h);
    draw_panel(panel);

    let title = if ui.show_settings {
        "Settings"
    } else {
        "Paused"
    };
    let tt = measure_text(title, None, 36, 1.0);
    draw_text(
        title,
        x + (panel_w - tt.width) * 0.5,
        y + 40.0,
        36.0,
        UI_ACCENT,
    );

    if !ui.show_settings {
        // Main pause buttons
        let bw = 260.0;
        let bh = 46.0;
        let gap = 18.0;
        let bx = x + (panel_w - bw) * 0.5;
        let mut by = y + 100.0;

        if ui_button(Rect::new(bx, by, bw, bh), "Continue", ButtonKind::Primary) {
            ui.paused = false;
            set_cursor_grab(true);
            show_mouse(false);
        }
        by += bh + gap;

        if ui_button(Rect::new(bx, by, bw, bh), "Settings", ButtonKind::Ghost) {
            ui.show_settings = true;
        }
        by += bh + gap;

        if ui_button(Rect::new(bx, by, bw, bh), "Leave Game", ButtonKind::Danger) {
            ui.net.take();
            *transition = Transition::ToMenu;
        }

        draw_text(
            "Tip: Press [Esc] to resume",
            x + (panel_w - 280.0) * 0.5,
            y + panel_h - 16.0,
            18.0,
            UI_TEXT_DIM,
        );
        return;
    }

    // SETTINGS VIEW — aligned using right-anchored controls so nothing spills out
    let margin = 32.0;
    let label_x = x + margin;
    let mut row_y = y + 102.0;

    // -------- Mouse Sensitivity row --------
    draw_text("Mouse Sensitivity", label_x, row_y, 22.0, UI_TEXT);

    // control sizes
    let btn_w = 44.0;
    let btn_h = 36.0;
    let value_w = 88.0;
    let gap = 8.0;
    let total_w = btn_w + gap + value_w + gap + btn_w;

    // anchor to right edge inside panel
    let controls_x = x + panel_w - margin - total_w;
    let controls_y = row_y - 26.0;

    // ASCII "-" to avoid missing glyph squares
    if ui_button(
        Rect::new(controls_x, controls_y, btn_w, btn_h),
        "-",
        ButtonKind::Ghost,
    ) {
        ui.scene.mouse_sens = (ui.scene.mouse_sens * 0.90).max(0.02);
    }

    let sens_val = format!("{:.2}", ui.scene.mouse_sens);
    let m = measure_text(&sens_val, None, 22, 1.0);
    let value_box = Rect::new(controls_x + btn_w + gap, controls_y, value_w, btn_h);
    draw_card(value_box, 2.0);
    draw_text(
        &sens_val,
        value_box.x + (value_w - m.width) * 0.5,
        value_box.y + 24.0,
        22.0,
        UI_TEXT,
    );

    if ui_button(
        Rect::new(
            controls_x + btn_w + gap + value_w + gap,
            controls_y,
            btn_w,
            btn_h,
        ),
        "+",
        ButtonKind::Ghost,
    ) {
        ui.scene.mouse_sens = (ui.scene.mouse_sens * 1.10).min(3.0);
    }

    row_y += 64.0;

    // -------- Crosshair row --------
    draw_text("Crosshair", label_x, row_y, 22.0, UI_TEXT);

    let cx_btn_w = 140.0;
    let cx_btn_h = 40.0;
    let cx_gap = 12.0;
    let cx_total = cx_btn_w * 2.0 + cx_gap;

    let cx_x = x + panel_w - margin - cx_total;
    let cx_y = row_y - 26.0;

    let current_is_plus = matches!(ui.scene.crosshair, CrosshairStyle::Plus);
    if ui_button(
        Rect::new(cx_x, cx_y, cx_btn_w, cx_btn_h),
        "Plus",
        if current_is_plus {
            ButtonKind::Primary
        } else {
            ButtonKind::Ghost
        },
    ) {
        ui.scene.crosshair = CrosshairStyle::Plus;
    }
    if ui_button(
        Rect::new(cx_x + cx_btn_w + cx_gap, cx_y, cx_btn_w, cx_btn_h),
        "Dot",
        if !current_is_plus {
            ButtonKind::Primary
        } else {
            ButtonKind::Ghost
        },
    ) {
        ui.scene.crosshair = CrosshairStyle::Dot;
    }

    row_y += 96.0;

    // -------- Back button (centered) --------
    let back_w = 200.0;
    let back_h = 44.0;
    if ui_button(
        Rect::new(x + (panel_w - back_w) * 0.5, row_y, back_w, back_h),
        "Back",
        ButtonKind::Ghost,
    ) {
        ui.show_settings = false;
    }

    draw_text(
        "Press [Esc] to go back",
        x + (panel_w - 220.0) * 0.5,
        y + panel_h - 16.0,
        18.0,
        UI_TEXT_DIM,
    );
}

// Small utility: color lerp (kept local to this file to avoid extra exports)
fn lerp(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}
