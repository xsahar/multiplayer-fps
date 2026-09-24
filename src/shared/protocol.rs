use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ClientMessage {
    Hello {
        username: String,
        protocol_version: u8,
    },
    Input {
        sequence: u64,
        turn: f32,
        move_forward: f32,
    },
    Shoot {
        sequence: u64,
    },
    ChangeLevel {
        level: u8,
    },
    Disconnect,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ServerMessage {
    Welcome {
        player_id: u32,
        level: u8,
    },
    Snapshot {
        tick: u64,
        players: Vec<PlayerState>,
    },
    Waiting {
        connected: usize,
        required: usize,
    },
    Error(String),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PlayerState {
    pub id: u32,
    pub username: String,
    pub level: u8,
    pub x: f32,
    pub y: f32,
    pub angle: f32,
    pub kills: u32,
    pub deaths: u32,
}
