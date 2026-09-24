use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Player {
    pub id: u32,
    pub username: String,
    pub x: f32,
    pub y: f32,
    pub angle: f32,
    pub level: u8,
    pub kills: u32,
    pub deaths: u32,
}

impl Player {
    pub fn new(id: u32, username: &str, x: f32, y: f32, level: u8) -> Self {
        Self {
            id,
            username: username.to_string(),
            x,
            y,
            angle: 0.0,
            level,
            kills: 0,
            deaths: 0,
        }
    }
}
