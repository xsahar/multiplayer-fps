use serde::{Deserialize, Serialize};

pub const DEFAULT_PORT: &str = "45000";
pub const MAX_PLAYERS: usize = 64;
pub const SERVER_TICK_HZ: u32 = 30;
pub const SNAPSHOT_HZ: u32 = 20;
pub const TILE_SIZE: f32 = 32.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LevelKind {
    Easy,   // Map 1
    Normal, // Map 2
    Hard,   // Map 3
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MapChoice {
    Map1,
    Map2,
    Map3,
    Random,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GamePhase {
    Lobby,
    Playing,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlayerSkin {
    Ninja,
    Alien,
    Robot,
    Pirate,
}

impl Default for PlayerSkin {
    fn default() -> Self {
        PlayerSkin::Ninja
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRequest {
    pub username: String,
    pub skin: PlayerSkin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinAccept {
    pub player_id: u32,
    pub width: u32,
    pub height: u32,
    pub level: LevelKind,
    pub tiles: Vec<u8>,
    pub phase: GamePhase,   // tells client if we’re in Lobby or Playing
    pub lobby_ms_left: u64, // only matters in Lobby
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRejected {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputState {
    pub forward: f32,
    pub strafe: f32,
    pub yaw_delta: f32,
    pub fire: bool,
    // kept for wire-compat but server ignores during Playing
    pub level_switch: Option<LevelKind>,
    pub generate_new: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientToServer {
    Join(JoinRequest),
    Input(InputState),
    Vote(MapChoice),
    Ping(u64),
    ReturnToLobby(String, PlayerSkin), // username, skin
    ChatMessage(String), // message text
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerNet {
    pub id: u32,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub yaw: f32,
    pub hp: i32,
    pub skin: PlayerSkin,

    // --- live stats for scoreboard ---
    pub kills: u32,
    pub deaths: u32,
    pub score: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub time_ms: u64,
    pub players: Vec<PlayerNet>,
    pub game_ms_left: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbyState {
    pub time_ms_left: u64,    // global timer countdown
    pub votes: [u32; 4],      // Map1, Map2, Map3, Random
    pub players: Vec<String>, // names in lobby
}

// winner / standings for end-of-round screen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreEntry {
    pub id: u32,
    pub name: String,
    pub kills: u32,
    pub deaths: u32,
    pub score: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMsg {
    pub player_name: String,
    pub message: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerToClient {
    JoinOk(JoinAccept),
    JoinRejected(JoinRejected),
    Snapshot(Snapshot),
    LevelChanged(JoinAccept), // also used as “game start” signal
    LobbyState(LobbyState),
    MatchOver { standings: Vec<ScoreEntry> },
    HitConfirm { target_id: u32 }, // NEW: confirm a hit (for hit-marker)
    Pong(u64),
    ServerFull,
    ChatMessage(ChatMsg),
}
