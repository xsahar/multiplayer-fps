use std::io;
use std::net::UdpSocket;

use crate::shared::protocol::{ClientMessage, ServerMessage};

pub struct Server {
    socket: UdpSocket,
    tick: u64,
}

impl Server {
    pub fn new() -> io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:8080")?;
        socket.set_nonblocking(true)?;
        Ok(Self { socket, tick: 0 })
    }

    pub fn run(&mut self) {
        let mut buffer = [0u8; 1024];

        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((len, addr)) => {
                    let payload = &buffer[..len];
                    if let Ok(message) = bincode::deserialize::<ClientMessage>(payload) {
                        let response = match message {
                            ClientMessage::Hello {
                                username,
                                protocol_version,
                            } => {
                                println!(
                                    "Client {} connected from {} with protocol {}",
                                    username, addr, protocol_version
                                );
                                ServerMessage::Welcome {
                                    player_id: 1,
                                    level: 1,
                                }
                            }
                            ClientMessage::Input {
                                sequence,
                                turn,
                                move_forward,
                            } => {
                                println!(
                                    "Input seq={} turn={} move={}",
                                    sequence, turn, move_forward
                                );
                                ServerMessage::Error("input accepted".to_string())
                            }
                            ClientMessage::Shoot { sequence } => {
                                println!("Shot seq={}", sequence);
                                ServerMessage::Error("shot accepted".to_string())
                            }
                            ClientMessage::ChangeLevel { level } => {
                                println!("Level change to {}", level);
                                ServerMessage::Welcome {
                                    player_id: 1,
                                    level,
                                }
                            }
                            ClientMessage::Disconnect => {
                                println!("Client disconnected from {}", addr);
                                ServerMessage::Error("disconnect".to_string())
                            }
                        };

                        let payload = bincode::serialize(&response).unwrap();
                        let _ = self.socket.send_to(&payload, addr);
                    }
                }
                Err(_) => {}
            }

            self.tick += 1;
        }
    }
}
