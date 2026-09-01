use crate::common::protocol::*;
use std::net::{SocketAddr, UdpSocket};
use std::time::Instant;

pub struct NetClient {
    socket: UdpSocket,
    server: SocketAddr,
    last_ping: Instant,
}

impl NetClient {
    pub fn connect(local_bind: &str, server: SocketAddr) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(local_bind)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            server,
            last_ping: Instant::now(),
        })
    }

    pub fn join(&self, username: String, skin: PlayerSkin) -> std::io::Result<()> {
        let msg = ClientToServer::Join(JoinRequest { username, skin });
        let buf = bincode::serialize(&msg).unwrap();
        self.socket.send_to(&buf, self.server)?;
        Ok(())
    }

    pub fn send_input(&self, input: &InputState) {
        if let Ok(buf) = bincode::serialize(&ClientToServer::Input(input.clone())) {
            let _ = self.socket.send_to(&buf, self.server);
        }
    }

    pub fn vote(&self, choice: MapChoice) {
        if let Ok(buf) = bincode::serialize(&ClientToServer::Vote(choice)) {
            let _ = self.socket.send_to(&buf, self.server);
        }
    }

    pub fn return_to_lobby(&self, username: String, skin: PlayerSkin) -> std::io::Result<()> {
        let msg = ClientToServer::ReturnToLobby(username, skin);
        let buf = bincode::serialize(&msg).unwrap();
        self.socket.send_to(&buf, self.server)?;
        Ok(())
    }

    pub fn send_chat(&self, text: String) {
        if let Ok(buf) = bincode::serialize(&ClientToServer::ChatMessage(text)) {
            let _ = self.socket.send_to(&buf, self.server);
        }
    }

    /// poll one packet; return Some(server message) if present
    pub fn poll(&self) -> Option<ServerToClient> {
        let mut buf = [0u8; 65535];
        if let Ok((amt, _)) = self.socket.recv_from(&mut buf) {
            if amt > 0 {
                if let Ok(msg) = bincode::deserialize::<ServerToClient>(&buf[..amt]) {
                    return Some(msg);
                }
            }
        }
        None
    }

    /// Send a Ping every ~500 ms so the server can keep us alive and measure latency.
    pub fn keepalive_tick(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_ping).as_millis() >= 500 {
            // the timestamp value is arbitrary; server just echoes it back
            let ts = now.elapsed().as_millis() as u64;
            if let Ok(buf) = bincode::serialize(&ClientToServer::Ping(ts)) {
                let _ = self.socket.send_to(&buf, self.server);
            }
            self.last_ping = now;
        }
    }
}
