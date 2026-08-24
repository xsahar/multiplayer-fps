use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum ClientMessage {
    Join { username: String },
    Input { change_x: i8, change_y: i8 },
    Ping { id: u64 },
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum ServerMessage {
    JoinAccepted { player_id: u32 },
    JoinRejected { reason: String },
    Snapshot { player_id: u32, x: f32, y: f32 },
    Pong { id: u64 },
    LobbyState { players: Vec<String>, countdown: u32 },
}

#[cfg(test)]
mod tests {
    use super::{ClientMessage, ServerMessage};

    #[test]
    fn join_message_survives_encoding_round_trip() {
        let original = ClientMessage::Join {
            username: String::from("Sahar"),
        };

        let encoded = bincode::serde::encode_to_vec(&original, bincode::config::standard())
            .expect("join message should encode");
        let (decoded, _bytes_read): (ClientMessage, usize) =
            bincode::serde::decode_from_slice(&encoded, bincode::config::standard())
                .expect("join message should decode");

        assert_eq!(decoded, original);
    }

    #[test]
    fn ping_pong_round_trip_survives_encoding() {
        let original = ServerMessage::Pong { id: 42 };

        let encoded = bincode::serde::encode_to_vec(&original, bincode::config::standard())
            .expect("pong message should encode");
        let (decoded, _bytes_read): (ServerMessage, usize) =
            bincode::serde::decode_from_slice(&encoded, bincode::config::standard())
                .expect("pong message should decode");

        assert_eq!(decoded, original);

        let client_original = ClientMessage::Ping { id: 42 };
        let client_encoded =
            bincode::serde::encode_to_vec(&client_original, bincode::config::standard())
                .expect("ping message should encode");
        let (client_decoded, _): (ClientMessage, usize) =
            bincode::serde::decode_from_slice(&client_encoded, bincode::config::standard())
                .expect("ping message should decode");

        assert_eq!(client_decoded, client_original);
    }
}