use std::net::UdpSocket;

use crate::shared::protocol::{ClientMessage, ServerMessage};

pub struct Client {
    socket: UdpSocket,
}

impl Client {
    pub fn new(server_ip: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        socket.connect(server_ip)?;
        Ok(Self { socket })
    }

    pub fn hello(&self, username: &str) -> std::io::Result<()> {
        let message = ClientMessage::Hello {
            username: username.to_string(),
            protocol_version: 1,
        };

        let bytes = bincode::serialize(&message).unwrap();
        self.socket.send(&bytes)?;
        Ok(())
    }

    pub fn read_server_message(&self) -> std::io::Result<Option<ServerMessage>> {
        let mut buf = [0u8; 4096];
        match self.socket.recv(&mut buf) {
            Ok(len) => {
                let trimmed = &buf[..len];
                let message = bincode::deserialize(trimmed).ok();
                Ok(message)
            }
            Err(ref err) if err.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(err) => Err(err),
        }
    }
}
