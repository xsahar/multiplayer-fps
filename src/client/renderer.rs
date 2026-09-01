use crate::common::protocol::*;
use macroquad::prelude::*;
use std::collections::HashMap;

/// -- small color helpers
#[inline]
pub fn col(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::new(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    )
}

// Cyberpunk HUD accents, matching the menu's neon theme (see UI_ACCENT / UI_ACCENT2 in bin/client.rs).
const NEON_MAGENTA: Color = Color::new(1.00, 0.10, 0.65, 1.00);
const NEON_CYAN: Color = Color::new(0.05, 0.95, 1.00, 1.00);
const NEON_TEXT_DIM: Color = Color::new(0.62, 0.70, 0.85, 1.00);

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

#[inline]
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::new(
        lerp(a.r, b.r, t),
        lerp(a.g, b.g, t),
        lerp(a.b, b.b, t),
        lerp(a.a, b.a, t),
    )
}

#[inline]
fn angle_lerp(a: f32, b: f32, t: f32) -> f32 {
    let mut diff = (b - a) % std::f32::consts::TAU;
    if diff > std::f32::consts::PI { diff -= 2.0 * std::f32::consts::PI; }
    else if diff < -std::f32::consts::PI { diff += 2.0 * std::f32::consts::PI; }
    a + diff * t
}

#[inline]
fn fog(c: Color, dist: f32, density: f32) -> Color {
    let f = (-(dist * density)).exp().clamp(0.0, 1.0);
    let bg = col(24, 26, 30, 255);
    lerp_color(bg, c, f)
}

fn draw_gradient_rect(x: f32, y: f32, w: f32, h: f32, c0: Color, c1: Color) {
    let steps = h.max(1.0) as i32;
    for i in 0..steps {
        let t = i as f32 / steps.max(1) as f32;
        let cc = lerp_color(c0, c1, t);
        draw_line(x, y + i as f32, x + w, y + i as f32, 1.0, cc);
    }
}

#[derive(Clone, Default)]
pub struct PlayerDraw {
    pub id: u32,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub yaw: f32,
    pub hp: i32,
    pub kills: u32,
    pub deaths: u32,
    pub score: i32,
    pub skin: PlayerSkin,
}

// ---------- Shooting FX types ----------
#[derive(Clone)]
struct Tracer {
    start_ms: f64,
    life_ms: f64,
    yaw: f32,
    len: f32, // in world tiles (length until wall)
}

#[derive(Clone)]
struct ScreenFlash {
    start_ms: f64,
    life_ms: f64,
}

// NEW: Crosshair style (Plus or Dot)
#[derive(Clone, Copy, Debug)]
pub enum CrosshairStyle {
    Plus,
    Dot,
}

pub struct Scene {
    pub maze_w: u32,
    pub maze_h: u32,
    pub tiles: Vec<u8>,
    pub me_id: u32,

    prev_players: HashMap<u32, PlayerDraw>,
    curr_players: HashMap<u32, PlayerDraw>,
    t_prev_ms: u64,
    t_curr_ms: u64,
    curr_local_recv_ms: f64,

    fps_smooth: f32,

    pub input_forward: f32,
    pub input_strafe: f32,

    pub mouse_sens: f32,
    fire_prev: bool,
    fire_just: bool,

    fire_now: bool,
    last_fire_fx_ms: f64,

    pub fov: f32,
    pub game_ms_left: u64,

    tracers: Vec<Tracer>,
    flash: Option<ScreenFlash>,
    hit_marker_until_ms: f64,

    pub crosshair: CrosshairStyle,
}

/// Shared billboard geometry passed to each skin's body-drawing routine.
struct ActorGeo<'a> {
    screen_x: f32,
    dist: f32,
    hp: i32,
    w: f32,
    top: f32,
    bottom: f32,
    left: f32,
    right: f32,
    x0: i32,
    x1: i32,
    sprite_w: f32,
    sprite_h: f32,
    depth: &'a Vec<f32>,
    visible: bool,
    outline: Color,
}

impl Scene {
    pub fn new(me_id: u32, w: u32, h: u32, tiles: Vec<u8>) -> Self {
        Self {
            maze_w: w,
            maze_h: h,
            tiles,
            me_id,
            prev_players: HashMap::new(),
            curr_players: HashMap::new(),
            t_prev_ms: 0,
            t_curr_ms: 0,
            curr_local_recv_ms: 0.0,
            fps_smooth: 0.0,
            input_forward: 0.0,
            input_strafe: 0.0,
            mouse_sens: 0.45,
            fire_prev: false,
            fire_just: false,
            fire_now: false,
            last_fire_fx_ms: 0.0,
            fov: std::f32::consts::FRAC_PI_2,
            game_ms_left: 0,
            tracers: Vec::new(),
            flash: None,
            hit_marker_until_ms: 0.0,
            crosshair: CrosshairStyle::Plus,
        }
    }

    #[inline]
    fn idx(&self, x: u32, y: u32) -> usize { (y * self.maze_w + x) as usize }

    pub fn apply_join(&mut self, accept: &JoinAccept, my_id: u32) {
        self.maze_w = accept.width;
        self.maze_h = accept.height;
        self.tiles = accept.tiles.clone();
        self.me_id = my_id;

        self.prev_players.clear();
        self.curr_players.clear();
        self.t_prev_ms = 0;
        self.t_curr_ms = 0;
        self.curr_local_recv_ms = (get_time() * 1000.0) as f64;

        self.input_forward = 0.0;
        self.input_strafe = 0.0;
        self.tracers.clear();
        self.flash = None;
        self.hit_marker_until_ms = 0.0;
        self.fire_prev = false;
        self.fire_just = false;
        self.fire_now = false;
        self.last_fire_fx_ms = 0.0;
        // keep user prefs (sens/crosshair)
    }

    fn draw_game_timer(&self) {
        if self.game_ms_left == 0 { return; }
        let secs = ((self.game_ms_left as f32) / 1000.0).ceil().max(0.0) as i32;
        let label = format!("Time left: {}s", secs);
        let t = measure_text(&label, None, 28, 1.0);
        let x = (screen_width() - t.width) * 0.5;
        draw_text(&label, x + 1.0, 36.0 + 1.0, 28.0, BLACK);
        draw_text(&label, x, 36.0, 28.0, NEON_MAGENTA);
    }

    pub fn apply_snapshot(&mut self, snap: &Snapshot) {
        if self.curr_players.is_empty() {
            self.curr_players = snap.players.iter().map(|p| (p.id, PlayerDraw {
                id: p.id, name: p.name.clone(), x: p.x, y: p.y, yaw: p.yaw,
                hp: p.hp, kills: p.kills, deaths: p.deaths, score: p.score, skin: p.skin,
            })).collect();
            self.prev_players = self.curr_players.clone();
            self.t_curr_ms = snap.time_ms;
            self.t_prev_ms = snap.time_ms;
            self.curr_local_recv_ms = (get_time() * 1000.0) as f64;
            self.game_ms_left = snap.game_ms_left;
            return;
        }

        self.prev_players = std::mem::take(&mut self.curr_players);
        self.curr_players = snap.players.iter().map(|p| (p.id, PlayerDraw {
            id: p.id, name: p.name.clone(), x: p.x, y: p.y, yaw: p.yaw,
            hp: p.hp, kills: p.kills, deaths: p.deaths, score: p.score, skin: p.skin,
        })).collect();

        self.t_prev_ms = self.t_curr_ms;
        self.t_curr_ms = snap.time_ms;
        self.curr_local_recv_ms = (get_time() * 1000.0) as f64;
        self.game_ms_left = snap.game_ms_left;
    }

    fn interpolate_players(&self) -> Vec<PlayerDraw> {
        let now_local_ms = (get_time() * 1000.0) as f64;
        let server_dt = (self.t_curr_ms as i64 - self.t_prev_ms as i64).max(1) as f64;
        let since_recv = (now_local_ms - self.curr_local_recv_ms).max(0.0);

        let mut alpha = (server_dt - since_recv).max(0.0) / server_dt;
        alpha = 1.0 - alpha;
        let alpha = alpha.clamp(0.0, 1.0) as f32;

        let mut out = Vec::with_capacity(self.curr_players.len());
        for (&id, b) in self.curr_players.iter() {
            let a = self.prev_players.get(&id).unwrap_or(b);
            out.push(PlayerDraw {
                id,
                name: b.name.clone(),
                x: lerp(a.x, b.x, alpha),
                y: lerp(a.y, b.y, alpha),
                yaw: angle_lerp(a.yaw, b.yaw, alpha),
                hp: b.hp, kills: b.kills, deaths: b.deaths, score: b.score, skin: b.skin,
            });
        }
        out
    }

    pub fn build_input(&mut self) -> InputState {
        if is_key_pressed(KeyCode::RightBracket) { self.mouse_sens = (self.mouse_sens * 1.10).min(3.0); }
        if is_key_pressed(KeyCode::LeftBracket)  { self.mouse_sens = (self.mouse_sens * 0.90).max(0.02); }
        if is_key_pressed(KeyCode::Backslash)    { self.mouse_sens = 0.45; }

        let target_forward = (is_key_down(KeyCode::W) || is_key_down(KeyCode::Up)) as i32
            - (is_key_down(KeyCode::S) || is_key_down(KeyCode::Down)) as i32;
        let target_strafe  = (is_key_down(KeyCode::D)) as i32 - (is_key_down(KeyCode::A)) as i32;

        let smooth = 0.22;
        self.input_forward = lerp(self.input_forward, target_forward as f32, smooth);
        self.input_strafe  = lerp(self.input_strafe,  target_strafe  as f32, smooth);
        if self.input_forward.abs() < 0.02 { self.input_forward = 0.0; }
        if self.input_strafe.abs()  < 0.02 { self.input_strafe  = 0.0; }

        // Inverted sign so moving mouse right rotates right
        let dx = mouse_delta_position().x;
        let mut yaw_delta = -dx * self.mouse_sens;

        let rotate_right = is_key_down(KeyCode::Right) || is_key_down(KeyCode::E);
        let rotate_left  = is_key_down(KeyCode::Left)  || is_key_down(KeyCode::Q);
        if rotate_right || rotate_left {
            yaw_delta += ((rotate_right as i32 - rotate_left as i32) as f32) * 0.25;
        }
        yaw_delta = yaw_delta.clamp(-5.0, 5.0);

        let fire_now = is_mouse_button_down(MouseButton::Left) || is_key_down(KeyCode::Space);
        self.fire_just = fire_now && !self.fire_prev;
        self.fire_prev = fire_now;
        self.fire_now = fire_now;

        InputState {
            forward: self.input_forward,
            strafe:  self.input_strafe,
            yaw_delta,
            fire: fire_now,
            level_switch: None,
            generate_new: false,
        }
    }

    pub fn fire_just(&self) -> bool { self.fire_just }

    pub fn tick_fire_fx(&mut self) {
        if !self.fire_now { return; }
        let now = get_time() * 1000.0;
        const FIRE_FX_INTERVAL_MS: f64 = 120.0;
        if now - self.last_fire_fx_ms < FIRE_FX_INTERVAL_MS { return; }
        if let Some((px, py, yaw)) = self.my_pose() {
            let len = self.fire_raycast_length(px, py, yaw, 30.0);
            self.spawn_shot_fx(yaw, len);
            self.last_fire_fx_ms = now;
        }
    }

    pub fn my_pose(&self) -> Option<(f32, f32, f32)> {
        self.curr_players.get(&self.me_id).map(|p| (p.x, p.y, p.yaw))
    }

    pub fn fire_raycast_length(&self, px: f32, py: f32, yaw: f32, max_len: f32) -> f32 {
        let (dist, _) = self.raycast(px, py, yaw);
        dist.min(max_len)
    }

    pub fn spawn_shot_fx(&mut self, yaw: f32, len: f32) {
        let now = get_time() * 1000.0;
        self.flash = Some(ScreenFlash { start_ms: now, life_ms: 60.0 });
        self.tracers.push(Tracer { start_ms: now, life_ms: 120.0, yaw, len });
    }

    pub fn on_hit_confirm(&mut self) {
        self.hit_marker_until_ms = (get_time() * 1000.0) + 120.0;
    }

    pub fn draw(&mut self) {
        let inst = get_fps() as f32;
        self.fps_smooth = if self.fps_smooth == 0.0 { inst } else { self.fps_smooth * 0.9 + inst * 0.1 };

        let players_now = self.interpolate_players();

        // HUD left
        if let Some(me) = players_now.iter().find(|p| p.id == self.me_id) {
            draw_text(&format!("Score: {}", me.score), 16.0, 72.0, 28.0, NEON_MAGENTA);
            draw_text(&format!("K/D: {}/{}", me.kills, me.deaths), 16.0, 96.0, 22.0, NEON_CYAN);
        }

        let (mx, my, my_yaw) = players_now
            .iter()
            .find(|p| p.id == self.me_id)
            .map(|p| (p.x, p.y, p.yaw))
            .unwrap_or((1.5, 1.5, 0.0));

        let depth = self.draw_first_person(mx, my, my_yaw);

        // FX layer
        self.draw_fx();

        self.draw_crosshair();

        draw_text(&format!("FPS: {:.0}", self.fps_smooth), 16.0, 24.0, 28.0, NEON_TEXT_DIM);
        draw_text(&format!("Sens: {:.2}  ([ / ] to change, \\ to reset)", self.mouse_sens), 16.0, 48.0, 22.0, NEON_TEXT_DIM);

        self.draw_minimap(&players_now);

        self.draw_game_timer();

        self.draw_sprites(mx, my, my_yaw, &depth, &players_now);

        if is_key_down(KeyCode::Tab) {
            self.draw_scoreboard(&players_now);
        }
    }

    fn draw_first_person(&self, px: f32, py: f32, yaw: f32) -> Vec<f32> {
        let w = screen_width().max(1.0);
        let h = screen_height().max(1.0);

        let sky_top = col(58, 66, 80, 255);
        let sky_bot = col(32, 36, 44, 255);
        let floor_top = col(66, 70, 78, 255);
        let floor_bot = col(30, 32, 38, 255);

        draw_gradient_rect(0.0, 0.0, w, h * 0.5, sky_top, sky_bot);
        draw_gradient_rect(0.0, h * 0.5, w, h * 0.5, floor_top, floor_bot);

        let num_cols = w as i32;
        let half_fov = self.fov * 0.5;
        let ray_step = self.fov / (num_cols as f32);

        let mut depth = vec![50.0; num_cols as usize];

        for xcol in 0..num_cols {
            let angle = yaw - half_fov + (xcol as f32) * ray_step;
            let (hit_dist, side) = self.raycast(px, py, angle);

            let perp = (hit_dist * (angle - yaw).cos().abs()).max(0.0001);
            depth[xcol as usize] = perp;

            let mut line_h = (h / perp) * 0.95;
            if line_h > h * 1.5 { line_h = h * 1.5; }

            let top = (h * 0.5 - line_h * 0.5).max(0.0);
            let bottom = (top + line_h).min(h);

            let base = if side == 0 { col(190, 190, 190, 255) } else { col(160, 160, 160, 255) };
            let shaded = fog(base, perp, 0.08);

            draw_line(xcol as f32, top, xcol as f32, bottom, 1.2, shaded);
        }

        depth
    }

    fn raycast(&self, px: f32, py: f32, angle: f32) -> (f32, i32) {
        let dir_x = angle.cos();
        let dir_y = angle.sin();

        let mut map_x = px.floor() as i32;
        let mut map_y = py.floor() as i32;

        let delta_dist_x = if dir_x.abs() < 1e-6 { 1e30 } else { (1.0 / dir_x).abs() };
        let delta_dist_y = if dir_y.abs() < 1e-6 { 1e30 } else { (1.0 / dir_y).abs() };

        let step_x: i32;
        let step_y: i32;

        let mut side = 0;
        let mut side_dist_x;
        let mut side_dist_y;

        if dir_x < 0.0 {
            step_x = -1;
            side_dist_x = (px - map_x as f32) * delta_dist_x;
        } else {
            step_x = 1;
            side_dist_x = ((map_x as f32 + 1.0) - px) * delta_dist_x;
        }
        if dir_y < 0.0 {
            step_y = -1;
            side_dist_y = (py - map_y as f32) * delta_dist_y;
        } else {
            step_y = 1;
            side_dist_y = ((map_y as f32 + 1.0) - py) * delta_dist_y;
        }

        for _ in 0..2048 {
            if side_dist_x < side_dist_y {
                side_dist_x += delta_dist_x;
                map_x += step_x;
                side = 0;
            } else {
                side_dist_y += delta_dist_y;
                map_y += step_y;
                side = 1;
            }
            if !self.in_bounds(map_x, map_y) { break; }
            if self.tiles[self.idx(map_x as u32, map_y as u32)] == 1 {
                let dist = if side == 0 {
                    (map_x as f32 - px + (1 - step_x) as f32 * 0.5) / dir_x
                } else {
                    (map_y as f32 - py + (1 - step_y) as f32 * 0.5) / dir_y
                };
                return (dist.abs().max(0.0001), side);
            }
        }
        (50.0, side)
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.maze_w && (y as u32) < self.maze_h
    }

    fn draw_minimap(&self, players_now: &Vec<PlayerDraw>) {
        let mini_tile = 4.5;
        let pad = 12.0;
        let mm_w_px = self.maze_w as f32 * mini_tile;
        let mm_h_px = self.maze_h as f32 * mini_tile;
        let origin_x = screen_width() - mm_w_px - pad;
        let origin_y = pad;

        draw_rectangle_lines(origin_x - 2.0, origin_y - 2.0, mm_w_px + 4.0, mm_h_px + 4.0, 2.0, Color::new(NEON_CYAN.r, NEON_CYAN.g, NEON_CYAN.b, 0.8));

        for y in 0..self.maze_h {
            for x in 0..self.maze_w {
                let tile = self.tiles[self.idx(x, y)];
                if tile == 1 {
                    let rx = origin_x + x as f32 * mini_tile;
                    let ry = origin_y + y as f32 * mini_tile;
                    draw_rectangle(rx, ry, mini_tile, mini_tile, col(90, 100, 110, 255));
                }
            }
        }
        for p in players_now {
            let rx = origin_x + p.x * mini_tile;
            let ry = origin_y + p.y * mini_tile;
            let colp = if p.id == self.me_id { NEON_MAGENTA } else { col(230, 90, 85, 255) };
            draw_rectangle(rx - 1.5, ry - 1.5, 3.0, 3.0, colp);
        }
    }

    // === billboard sprite dispatcher for other players ===
    #[allow(clippy::too_many_arguments)]
    fn draw_actor_billboard(
        &self,
        screen_x: f32,
        dist: f32,
        hp: i32,
        name: &str,
        depth: &Vec<f32>,
        width: f32,
        height: f32,
        skin: PlayerSkin,
    ) {
        let w = width;
        let h = height;

        let sprite_h = (h / dist.max(0.001)) * 0.75;
        let sprite_w = (sprite_h * 0.48).clamp(6.0, w * 0.28);

        let top = (h * 0.5 - sprite_h * 0.72).max(0.0);
        let bottom = (top + sprite_h).min(h);
        let left = (screen_x - sprite_w * 0.5).max(0.0);
        let right = (screen_x + sprite_w * 0.5).min(w);

        let x0 = left as i32;
        let x1 = right as i32;
        let center_idx = screen_x.clamp(0.0, w - 1.0) as usize;
        let visible = center_idx < depth.len() && dist < depth[center_idx] - 0.01;
        let outline = fog(col(0, 0, 0, 200), dist, 0.05);

        // shadow
        let shadow_y = bottom.min(h - 4.0);
        let shadow_w = sprite_w * 0.8;
        let shadow_h = 4.0 + (8.0 / dist.max(1.0));
        draw_ellipse(screen_x, shadow_y, shadow_w * 0.5, shadow_h, 0.0, col(0, 0, 0, 90));

        let geo = ActorGeo {
            screen_x, dist, hp, w, top, bottom, left, right, x0, x1,
            sprite_w, sprite_h, depth, visible, outline,
        };

        match skin {
            PlayerSkin::Ninja => self.draw_ninja_body(&geo),
            PlayerSkin::Alien => self.draw_alien_body(&geo),
            PlayerSkin::Robot => self.draw_robot_body(&geo),
            PlayerSkin::Pirate => self.draw_pirate_body(&geo),
        }

        // weapon slab (held in front of the body)
        let chest_y0 = top + sprite_h * 0.60;
        let gun_w = sprite_w * 0.55;
        let gun_h = sprite_h * 0.10;
        let gx = screen_x - gun_w * 0.5;
        if visible {
            draw_rectangle(gx, chest_y0, gun_w, gun_h, fog(col(60, 60, 70, 255), dist, 0.08));
            draw_rectangle_lines(gx, chest_y0, gun_w, gun_h, 1.0, outline);
        }

        // name + HP bar
        let name_size = 16.0;
        let nt = measure_text(name, None, name_size as u16, 1.0);
        let label_y = (top - 10.0).max(14.0);
        draw_text(name, screen_x - nt.width * 0.5 + 1.0, label_y + 1.0, name_size, BLACK);
        draw_text(name, screen_x - nt.width * 0.5, label_y, name_size, WHITE);

        let bar_w = sprite_w.max(28.0);
        let bar_h = 6.0;
        let bx = screen_x - bar_w * 0.5;
        let by = label_y + 6.0;
        draw_rectangle(bx - 1.0, by - 1.0, bar_w + 2.0, bar_h + 2.0, col(0, 0, 0, 120));
        let hp_ratio = (hp as f32 / 100.0).clamp(0.0, 1.0);
        let hp_col = lerp_color(col(220, 60, 40, 255), col(60, 200, 90, 255), hp_ratio);
        draw_rectangle(bx, by, bar_w * hp_ratio, bar_h, fog(hp_col, dist, 0.10));
        draw_rectangle_lines(bx, by, bar_w, bar_h, 1.0, col(255, 255, 255, 90));
    }

    // === Minion body: banana-yellow capsule, denim overalls, single goggle eye ===
    fn draw_ninja_body(&self, g: &ActorGeo) {
        let (screen_x, dist, hp) = (g.screen_x, g.dist, g.hp);
        let (top, bottom, left, right) = (g.top, g.bottom, g.left, g.right);
        let (sprite_w, sprite_h) = (g.sprite_w, g.sprite_h);
        let outline = g.outline;

        let t = (1.0 - (hp as f32 / 100.0)).clamp(0.0, 1.0);
        let skin = lerp_color(col(250, 210, 30, 255), col(210, 90, 40, 255), t);
        let denim = col(58, 92, 168, 255);
        let denim_dark = col(40, 68, 130, 255);
        let steel = col(150, 155, 160, 255);

        let overalls_top = top + sprite_h * 0.55;
        for sx in g.x0..=g.x1 {
            let idx = sx as usize;
            if idx >= g.depth.len() || dist >= g.depth[idx] - 0.01 { continue; }
            let fx = sx as f32;
            let edge = ((fx - screen_x).abs() / (sprite_w * 0.5)).clamp(0.0, 1.0);
            let shade = lerp_color(skin, col(0, 0, 0, 255), edge * 0.22);
            draw_line(fx, top, fx, overalls_top, 1.2, fog(shade, dist, 0.08));

            let overalls_shade = lerp_color(denim, denim_dark, edge);
            draw_line(fx, overalls_top, fx, bottom, 1.2, fog(overalls_shade, dist, 0.08));
        }

        if !g.visible { return; }

        // rounded head/body cap
        draw_ellipse(screen_x, top, sprite_w * 0.5, sprite_h * 0.06, 0.0, fog(skin, dist, 0.08));
        draw_rectangle_lines(left, top, right - left, bottom - top, 1.2, outline);

        // denim bib + shoulder straps
        let bib_w = sprite_w * 0.5;
        let bib_x = screen_x - bib_w * 0.5;
        draw_rectangle(bib_x, overalls_top - sprite_h * 0.05, bib_w, sprite_h * 0.1, fog(denim_dark, dist, 0.08));
        let strap_w = sprite_w * 0.12;
        draw_rectangle(screen_x - sprite_w * 0.3, top + sprite_h * 0.32, strap_w, overalls_top - (top + sprite_h * 0.32), fog(denim, dist, 0.08));
        draw_rectangle(screen_x + sprite_w * 0.3 - strap_w, top + sprite_h * 0.32, strap_w, overalls_top - (top + sprite_h * 0.32), fog(denim, dist, 0.08));

        // pocket buckle
        let buckle_r = sprite_w * 0.07;
        draw_circle(screen_x, overalls_top, buckle_r, fog(col(220, 200, 60, 255), dist, 0.08));
        draw_circle_lines(screen_x, overalls_top, buckle_r, 1.0, outline);

        // a couple of black hair spikes
        for i in -1i32..=1 {
            let hx = screen_x + (i as f32) * sprite_w * 0.14;
            let hy0 = top - sprite_h * 0.02;
            let hy1 = top - sprite_h * 0.09 - (i.abs() as f32) * sprite_h * 0.02;
            draw_line(hx, hy0, hx, hy1, 1.6, fog(col(20, 20, 20, 255), dist, 0.08));
        }

        // goggle strap band across the head
        let goggle_y = top + sprite_h * 0.20;
        let goggle_band_h = sprite_h * 0.07;
        draw_rectangle(left, goggle_y - goggle_band_h * 0.5, right - left, goggle_band_h, fog(col(30, 30, 30, 230), dist, 0.06));

        // one big goggle eye (classic single-eye minion look)
        let eye_r = sprite_w * 0.22;
        draw_circle(screen_x, goggle_y, eye_r, fog(steel, dist, 0.08));
        draw_circle_lines(screen_x, goggle_y, eye_r, 1.4, outline);
        draw_circle(screen_x, goggle_y, eye_r * 0.62, fog(col(240, 240, 235, 255), dist, 0.06));
        let pupil_dir = ((screen_x - g.w * 0.5) / (g.w * 0.5)).clamp(-1.0, 1.0);
        draw_circle(screen_x + pupil_dir * eye_r * 0.22, goggle_y, eye_r * 0.30, fog(col(30, 25, 20, 255), dist, 0.04));

        // mouth grin
        let mouth_y = top + sprite_h * 0.34;
        draw_line(screen_x - sprite_w * 0.16, mouth_y, screen_x + sprite_w * 0.16, mouth_y + sprite_h * 0.02, 1.4, fog(col(40, 20, 15, 255), dist, 0.08));
    }

    // === Steve body (Minecraft-style): blocky skin/hair/shirt/pants bands ===
    fn draw_alien_body(&self, g: &ActorGeo) {
        let (screen_x, dist, hp) = (g.screen_x, g.dist, g.hp);
        let (top, bottom, left, right) = (g.top, g.bottom, g.left, g.right);
        let (sprite_w, sprite_h) = (g.sprite_w, g.sprite_h);
        let outline = g.outline;

        // hp tints the shirt toward a battle-worn red
        let t = (1.0 - (hp as f32 / 100.0)).clamp(0.0, 1.0);
        let shirt = lerp_color(col(60, 160, 170, 255), col(150, 60, 55, 255), t);
        let shirt_dark = lerp_color(col(40, 120, 130, 255), col(110, 40, 40, 255), t);
        let pants = col(58, 78, 140, 255);
        let pants_dark = col(40, 56, 108, 255);
        let hair = col(70, 45, 25, 255);
        let face = col(225, 175, 135, 255);

        let hair_bot = top + sprite_h * 0.06;
        let face_bot = top + sprite_h * 0.22;
        let shirt_bot = top + sprite_h * 0.58;

        for sx in g.x0..=g.x1 {
            let idx = sx as usize;
            if idx >= g.depth.len() || dist >= g.depth[idx] - 0.01 { continue; }
            let fx = sx as f32;
            let edge = ((fx - screen_x).abs() / (sprite_w * 0.5)).clamp(0.0, 1.0);

            draw_line(fx, top, fx, hair_bot, 1.2, fog(hair, dist, 0.08));
            draw_line(fx, hair_bot, fx, face_bot, 1.2, fog(face, dist, 0.08));

            let shirt_shade = lerp_color(shirt, shirt_dark, edge);
            draw_line(fx, face_bot, fx, shirt_bot, 1.2, fog(shirt_shade, dist, 0.08));

            let pants_shade = lerp_color(pants, pants_dark, edge);
            draw_line(fx, shirt_bot, fx, bottom, 1.2, fog(pants_shade, dist, 0.08));
        }

        if !g.visible { return; }

        draw_rectangle_lines(left, top, right - left, bottom - top, 1.2, outline);

        // blocky square eyes
        let eye_y = top + sprite_h * 0.15;
        let eye_r = sprite_w * 0.09;
        draw_rectangle(screen_x - sprite_w * 0.22 - eye_r, eye_y - eye_r, eye_r * 2.0, eye_r * 2.0, fog(col(30, 25, 20, 255), dist, 0.06));
        draw_rectangle(screen_x + sprite_w * 0.22 - eye_r, eye_y - eye_r, eye_r * 2.0, eye_r * 2.0, fog(col(30, 25, 20, 255), dist, 0.06));

        // simple nose block
        let nose_w = sprite_w * 0.08;
        let nose_h = sprite_h * 0.035;
        draw_rectangle(screen_x - nose_w * 0.5, eye_y + eye_r, nose_w, nose_h, fog(col(190, 140, 110, 255), dist, 0.06));

        // belt line between shirt and pants
        draw_rectangle(left, shirt_bot - sprite_h * 0.015, right - left, sprite_h * 0.03, fog(col(40, 30, 25, 255), dist, 0.06));
    }

    // === Cat body: fur capsule, triangle ears, whiskers, slit-pupil eyes ===
    fn draw_robot_body(&self, g: &ActorGeo) {
        let (screen_x, dist, hp) = (g.screen_x, g.dist, g.hp);
        let (top, bottom, left, right) = (g.top, g.bottom, g.left, g.right);
        let (sprite_w, sprite_h) = (g.sprite_w, g.sprite_h);
        let outline = g.outline;

        // hp tints the fur toward a duller gray as damage mounts
        let t = (1.0 - (hp as f32 / 100.0)).clamp(0.0, 1.0);
        let fur = lerp_color(col(235, 150, 60, 255), col(120, 100, 95, 255), t);
        let fur_dark = lerp_color(col(190, 110, 40, 255), col(90, 75, 70, 255), t);

        for sx in g.x0..=g.x1 {
            let idx = sx as usize;
            if idx >= g.depth.len() || dist >= g.depth[idx] - 0.01 { continue; }
            let fx = sx as f32;
            let edge = ((fx - screen_x).abs() / (sprite_w * 0.5)).clamp(0.0, 1.0);
            let shade = lerp_color(fur, fur_dark, edge);
            draw_line(fx, top, fx, bottom, 1.2, fog(shade, dist, 0.08));
        }

        if !g.visible { return; }

        draw_ellipse(screen_x, top, sprite_w * 0.5, sprite_h * 0.06, 0.0, fog(fur, dist, 0.08));
        draw_rectangle_lines(left, top, right - left, bottom - top, 1.2, outline);

        // triangle ears
        let ear_h = sprite_h * 0.10;
        let ear_w = sprite_w * 0.28;
        let ear_y = top - sprite_h * 0.01;
        draw_triangle(
            vec2(screen_x - sprite_w * 0.38, ear_y),
            vec2(screen_x - sprite_w * 0.38 + ear_w, ear_y),
            vec2(screen_x - sprite_w * 0.30, ear_y - ear_h),
            fog(fur_dark, dist, 0.08),
        );
        draw_triangle(
            vec2(screen_x + sprite_w * 0.38 - ear_w, ear_y),
            vec2(screen_x + sprite_w * 0.38, ear_y),
            vec2(screen_x + sprite_w * 0.30, ear_y - ear_h),
            fog(fur_dark, dist, 0.08),
        );

        // white muzzle patch
        let muzzle_y = top + sprite_h * 0.28;
        draw_ellipse(screen_x, muzzle_y, sprite_w * 0.24, sprite_h * 0.05, 0.0, fog(col(250, 245, 235, 255), dist, 0.06));

        // pink nose
        draw_triangle(
            vec2(screen_x - sprite_w * 0.05, muzzle_y - sprite_h * 0.02),
            vec2(screen_x + sprite_w * 0.05, muzzle_y - sprite_h * 0.02),
            vec2(screen_x, muzzle_y + sprite_h * 0.02),
            fog(col(230, 140, 150, 255), dist, 0.08),
        );

        // whiskers
        let whisker_y = muzzle_y;
        for i in 0..3 {
            let dy = (i as f32 - 1.0) * sprite_h * 0.02;
            draw_line(screen_x - sprite_w * 0.55, whisker_y + dy, screen_x - sprite_w * 0.15, whisker_y + dy * 0.4, 0.8, fog(col(255, 255, 255, 200), dist, 0.08));
            draw_line(screen_x + sprite_w * 0.15, whisker_y + dy * 0.4, screen_x + sprite_w * 0.55, whisker_y + dy, 0.8, fog(col(255, 255, 255, 200), dist, 0.08));
        }

        // green slit-pupil eyes
        let eye_y = top + sprite_h * 0.18;
        let eye_r = sprite_w * 0.13;
        for dir in [-1.0f32, 1.0] {
            let ex = screen_x + dir * sprite_w * 0.20;
            draw_circle(ex, eye_y, eye_r, fog(col(150, 220, 90, 255), dist, 0.08));
            draw_circle_lines(ex, eye_y, eye_r, 1.0, outline);
            draw_line(ex, eye_y - eye_r * 0.7, ex, eye_y + eye_r * 0.7, eye_r * 0.35, fog(col(20, 20, 20, 255), dist, 0.04));
        }
    }

    // === Ghost body (tactical operator): dark fatigues + a skull-print balaclava ===
    fn draw_pirate_body(&self, g: &ActorGeo) {
        let (screen_x, dist, hp) = (g.screen_x, g.dist, g.hp);
        let (top, bottom, left, right) = (g.top, g.bottom, g.left, g.right);
        let (sprite_w, sprite_h) = (g.sprite_w, g.sprite_h);
        let outline = g.outline;

        // hp tints the tactical gear toward a bloodied dark red as damage mounts
        let t = (1.0 - (hp as f32 / 100.0)).clamp(0.0, 1.0);
        let gear = lerp_color(col(58, 64, 58, 255), col(120, 40, 40, 255), t);
        let gear_dark = lerp_color(col(36, 40, 36, 255), col(80, 26, 26, 255), t);
        let mask = col(18, 18, 20, 255);
        let bone = col(225, 222, 214, 255);

        let mask_bot = top + sprite_h * 0.24;

        for sx in g.x0..=g.x1 {
            let idx = sx as usize;
            if idx >= g.depth.len() || dist >= g.depth[idx] - 0.01 { continue; }
            let fx = sx as f32;
            let edge = ((fx - screen_x).abs() / (sprite_w * 0.5)).clamp(0.0, 1.0);

            draw_line(fx, top, fx, mask_bot, 1.2, fog(mask, dist, 0.08));

            let gear_shade = lerp_color(gear, gear_dark, edge);
            draw_line(fx, mask_bot, fx, bottom, 1.2, fog(gear_shade, dist, 0.08));
        }

        if !g.visible { return; }

        draw_rectangle_lines(left, top, right - left, bottom - top, 1.2, outline);

        // skull-mask print over the balaclava: eye sockets, nose bridge, a row of teeth
        let eye_y = top + sprite_h * 0.11;
        let eye_r = sprite_w * 0.11;
        draw_ellipse(screen_x - sprite_w * 0.16, eye_y, eye_r, eye_r * 1.2, 0.0, fog(bone, dist, 0.06));
        draw_ellipse(screen_x + sprite_w * 0.16, eye_y, eye_r, eye_r * 1.2, 0.0, fog(bone, dist, 0.06));
        draw_circle(screen_x - sprite_w * 0.16, eye_y, eye_r * 0.5, fog(mask, dist, 0.04));
        draw_circle(screen_x + sprite_w * 0.16, eye_y, eye_r * 0.5, fog(mask, dist, 0.04));

        let nose_y = top + sprite_h * 0.17;
        draw_triangle(
            vec2(screen_x - sprite_w * 0.03, nose_y),
            vec2(screen_x + sprite_w * 0.03, nose_y),
            vec2(screen_x, nose_y + sprite_h * 0.03),
            fog(bone, dist, 0.06),
        );

        let teeth_y = mask_bot - sprite_h * 0.04;
        let teeth_w = sprite_w * 0.34;
        let tx0 = screen_x - teeth_w * 0.5;
        let tooth_n = 5i32;
        for i in 0..tooth_n {
            let tw = teeth_w / tooth_n as f32;
            let tx = tx0 + tw * i as f32;
            draw_rectangle(tx + 0.5, teeth_y, tw - 1.0, sprite_h * 0.03, fog(bone, dist, 0.06));
        }

        // tactical rig strap across the chest
        let rig_y = mask_bot + sprite_h * 0.14;
        draw_rectangle(left, rig_y, right - left, sprite_h * 0.05, fog(gear_dark, dist, 0.08));
    }

    fn draw_sprites(
        &self,
        px: f32,
        py: f32,
        yaw: f32,
        depth: &Vec<f32>,
        players_now: &Vec<PlayerDraw>,
    ) {
        let w = screen_width();
        let h = screen_height();

        let mut targets: Vec<_> = players_now
            .iter()
            .filter(|p| p.id != self.me_id)
            .map(|p| {
                let dx = p.x - px;
                let dy = p.y - py;
                let dist_euclid = (dx * dx + dy * dy).sqrt();
                let angle_to = dy.atan2(dx);
                let mut rel = angle_to - yaw;
                while rel > std::f32::consts::PI { rel -= 2.0 * std::f32::consts::PI; }
                while rel < -std::f32::consts::PI { rel += 2.0 * std::f32::consts::PI; }
                // Perpendicular (fisheye-corrected) distance, matching the units stored in
                // the wall depth buffer -- comparing raw Euclidean distance against that
                // buffer made off-center players (i.e. most of the screen) look occluded
                // by walls they weren't actually behind.
                let dist = (dist_euclid * rel.cos()).max(0.0001);
                (p.id, p.name.clone(), dist, rel, p.hp, p.skin)
            })
            .collect();

        // far->near
        targets.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        let half_fov = self.fov * 0.5;

        for (_id, name, dist, rel, hp, skin) in targets {
            if rel.abs() > half_fov * 1.2 { continue; }
            let screen_x = (0.5 + rel / self.fov) * w;
            self.draw_actor_billboard(screen_x, dist, hp, &name, depth, w, h, skin);
        }
    }

    fn draw_crosshair(&self) {
        let cx = screen_width() * 0.5;
        let cy = screen_height() * 0.5;

        match self.crosshair {
            CrosshairStyle::Plus => {
                let s = 8.0;
                let c = Color::new(NEON_CYAN.r, NEON_CYAN.g, NEON_CYAN.b, 0.86);
                draw_line(cx - s, cy, cx + s, cy, 2.0, c);
                draw_line(cx, cy - s, cx, cy + s, 2.0, c);
            }
            CrosshairStyle::Dot => {
                draw_circle(cx, cy, 2.0, Color::new(NEON_CYAN.r, NEON_CYAN.g, NEON_CYAN.b, 0.9));
            }
        }
    }

    // -------------- FX rendering --------------
    fn draw_fx(&mut self) {
        let now = get_time() * 1000.0;

        // Screen flash
        if let Some(f) = &self.flash {
            let t = ((now - f.start_ms) / f.life_ms).clamp(0.0, 1.0);
            if t < 1.0 {
                let alpha = (1.0 - t) as f32;
                let w = screen_width();
                let h = screen_height();
                draw_rectangle(0.0, 0.0, w, h, col(255, 255, 220, (120.0 * alpha) as u8));
            } else {
                self.flash = None;
            }
        }

        // Tracers
        self.tracers.retain(|tr| (now - tr.start_ms) <= tr.life_ms);
        for tr in &self.tracers {
            let t = ((now - tr.start_ms) / tr.life_ms).clamp(0.0, 1.0) as f32;
            let fade = 1.0 - t;
            let cx = screen_width() * 0.5;
            let cy = screen_height() * 0.5;

            let px_len = (screen_height() / tr.len.max(0.001)) * 0.6;
            let l = px_len.clamp(40.0, screen_height() * 0.45);

            let dir_x = tr.yaw.cos();
            let dir_y = tr.yaw.sin();

            let endx = cx + dir_x * l;
            let endy = cy + dir_y * l;

            let glow = col(255, 240, 160, (180.0 * fade) as u8);
            let core = col(255, 255, 255, (220.0 * fade) as u8);
            draw_line(cx, cy, endx, endy, 3.0, glow);
            draw_line(cx, cy, endx, endy, 1.5, core);

            draw_circle(cx, cy, 4.0 + 4.0 * fade, col(255, 230, 160, (160.0 * fade) as u8));
        }

        // Hit marker
        if now <= self.hit_marker_until_ms {
            let cx = screen_width() * 0.5;
            let cy = screen_height() * 0.5;
            let s = 12.0;
            let w = 2.0;
            let a = col(255, 255, 255, 220);
            draw_line(cx - s, cy - s, cx - s * 0.4, cy - s * 0.4, w, a);
            draw_line(cx + s, cy - s, cx + s * 0.4, cy - s * 0.4, w, a);
            draw_line(cx - s, cy + s, cx - s * 0.4, cy + s * 0.4, w, a);
            draw_line(cx + s, cy + s, cx + s * 0.4, cy + s * 0.4, w, a);
        }
    }

    fn draw_scoreboard(&self, players_now: &Vec<PlayerDraw>) {
        // Sort players
        let mut rows: Vec<_> = players_now.iter().cloned().collect();
        rows.sort_by(|a, b| {
            b.score.cmp(&a.score)
                .then(b.kills.cmp(&a.kills))
                .then(a.deaths.cmp(&b.deaths))
                .then(a.id.cmp(&b.id))
        });

        let w = screen_width();
        let h = screen_height();
        let bw = (w * 0.72).clamp(720.0, 1400.0);
        let bh = (rows.len() as f32 + 2.0) * 28.0 + 40.0 + 28.0;
        let x = (w - bw) * 0.5;
        let y = (h - bh) * 0.5;

        // panel with shadow
        draw_rectangle(x + 8.0, y + 8.0, bw, bh, col(0, 0, 0, 140));
        draw_rectangle(x, y, bw, bh, col(18, 10, 28, 230));
        draw_rectangle_lines(x, y, bw, bh, 2.0, Color::new(NEON_CYAN.r, NEON_CYAN.g, NEON_CYAN.b, 0.9));

        let mut yy = y + 36.0;
        let title = "Scoreboard";
        let tt = measure_text(title, None, 28, 1.0);
        draw_text(title, x + (bw - tt.width) * 0.5, y + 26.0, 28.0, NEON_MAGENTA);

        yy += 12.0;

        let headers = ["Player", "Kills", "Deaths", "Score"];
        let cols = [0.08, 0.56, 0.72, 0.86];
        for (i, htxt) in headers.iter().enumerate() {
            draw_text(htxt, x + bw * cols[i], yy, 22.0, NEON_TEXT_DIM);
        }
        yy += 8.0;
        draw_line(x + 12.0, yy, x + bw - 12.0, yy, 1.0, Color::new(NEON_CYAN.r, NEON_CYAN.g, NEON_CYAN.b, 0.5));
        yy += 24.0;

        for p in rows {
            let is_me = p.id == self.me_id;
            let name_col = if is_me { NEON_MAGENTA } else { WHITE };
            draw_text(&p.name, x + bw * cols[0], yy, 22.0, name_col);
            draw_text(&format!("{}", p.kills),  x + bw * cols[1], yy, 22.0, WHITE);
            draw_text(&format!("{}", p.deaths), x + bw * cols[2], yy, 22.0, WHITE);
            draw_text(&format!("{}", p.score),  x + bw * cols[3], yy, 22.0, WHITE);
            yy += 26.0;
        }

        let hint = "Hold [Tab] to view scoreboard";
        let th = measure_text(hint, None, 18, 1.0);
        draw_text(hint, x + (bw - th.width) * 0.5, y + bh - 12.0, 18.0, col(170, 180, 190, 255));
    }
}
