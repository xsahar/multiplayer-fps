use crate::common::protocol::PlayerSkin;

#[derive(Clone)]
pub struct Player {
    pub id: u32,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub yaw: f32,       // radians
    pub speed: f32,     // tiles per second
    pub hp: i32,        // 0..100
    pub last_shot: u64, // ms since epoch (server time)
    pub spawn_protection_until: u64, // ms - invulnerable until this time
    pub skin: PlayerSkin,

    // stats
    pub kills: u32,
    pub deaths: u32,
    pub score: i32, // total score (+10 per hit)
}

impl Player {
    pub fn new(id: u32, name: String, x: f32, y: f32, skin: PlayerSkin) -> Self {
        Self {
            id,
            name,
            x,
            y,
            yaw: 0.0,
            speed: 3.5,
            hp: 100,
            last_shot: 0,
            spawn_protection_until: 0,
            skin,
            kills: 0,
            deaths: 0,
            score: 0,
        }
    }

    pub fn alive(&self) -> bool {
        self.hp > 0
    }

    pub fn reset(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
        self.yaw = 0.0;
        self.hp = 100;
        self.last_shot = 0;
        self.spawn_protection_until = 0;
        // stats persist until round start reset
    }

    pub fn has_spawn_protection(&self, now_ms: u64) -> bool {
        now_ms < self.spawn_protection_until
    }

    pub fn reset_stats(&mut self) {
        self.kills = 0;
        self.deaths = 0;
        self.score = 0;
    }
}
