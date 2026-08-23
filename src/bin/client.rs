use multiplayer_fps::protocol::ClientMessage;
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
}