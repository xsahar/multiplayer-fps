use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum ClientMessage {
    Join { username: String },
    Input { change_x: i8, change_y: i8 },
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum ServerMessage {
    JoinAccepted { player_id: u32 },
    JoinRejected { reason: String },
}

#[cfg(test)]
mod tests {
    use super::ClientMessage;

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
}