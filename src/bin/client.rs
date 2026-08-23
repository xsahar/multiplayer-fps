use multiplayer_fps::protocol::{ClientMessage, ServerMessage};
use std::net::UdpSocket;

fn main() {

    let socket = UdpSocket::bind("127.0.0.1:0")
        .expect("Could not bind client socket");

    let message = ClientMessage::Join {
        username: String::from("Sahar"),
    };

    let bytes = bincode::serde::encode_to_vec(
        &message,
        bincode::config::standard(),
    )
    .expect("Could not encode join message");

    socket
        .send_to(&bytes, "127.0.0.1:34254")
        .expect("Could not send join message");

    println!("Join message sent");

    let mut buffer = [0u8; 1024];
    let (amount, _server_address) = socket
        .recv_from(&mut buffer)
        .expect("Could not receive server response");

    let (response, _bytes_read): (ServerMessage, usize) =
        bincode::serde::decode_from_slice(
            &buffer[..amount],
            bincode::config::standard(),
        )
        .expect("Could not decode server response");

    println!("Server response: {:?}", response);

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

    let (amount, _server_address) = socket
        .recv_from(&mut buffer)
        .expect("Could not receive snapshot");
    let (snapshot, _bytes_read): (ServerMessage, usize) =
        bincode::serde::decode_from_slice(
            &buffer[..amount],
            bincode::config::standard(),
        )
        .expect("Could not decode snapshot");

    println!("Server snapshot: {:?}", snapshot);
}