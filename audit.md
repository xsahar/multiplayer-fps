# Audit Status Tracker

## Project status

This repository currently contains a playable Rust UDP multiplayer maze-game prototype. The core graphical gameplay requirements are implemented; remote-network and sustained multiplayer validation remain.

## Verified status

- Server skeleton compiles and starts
- Client skeleton compiles and connects through UDP
- Protocol serialization works
- Collision logic works in tests
- Project passes `cargo check` and `cargo test`
- Graphical client includes an IP/port and username connection form
- First-person maze view, minimap, player markers, movement, and FPS HUD are implemented
- Three server-side maze levels are implemented and synchronized to the client
- Multiple local clients connect and receive synchronized snapshots

## Audit checklist with current status

### Functional

##### Try to run the game server
Status: Pass (scaffold level)

###### Does it compile and run without any warnings?
Status: Pass for the current project state

##### Try to run a client in the same computer as the server.
Status: Pass for the current project state

###### Does it compile and run without any warnings?
Status: Pass for the current project state

###### Does it ask for the IP address of the server?
Status: Pass; graphical connection form collects the server IP and port before connecting

##### Insert the IP address of the game server.
Status: Pass; submitted address is used for the UDP connection

###### Does the client manage to connect to the server?
Status: Yes, at the UDP handshake level

###### Does the client ask you for a username?
Status: Pass; graphical connection form collects the username

##### Insert your username.
Status: Pass; submitted username is sent in the initial handshake

###### Does the client initiate the graphical interface?
Status: Pass; Macroquad client opens after the connection form is submitted

###### Are you presented with a mini map of the maze?
Status: Pass; minimap is rendered in the client

###### Can you see your position in the mini map?
Status: Pass; local player is shown in green and remote same-level players in red

###### When you move around in the world, does your position update in the mini map?
Status: Pass; movement updates the authoritative position shown on the minimap

###### When you move around the maze, does the view of the camera update?
Status: Pass; first-person raycast view updates from position and angle

###### Is the frame rate displayed in the interface?
Status: Pass; FPS is displayed in the HUD

###### Is the frame rate of the game higher than 50 fps?
Status: Pass in local runtime checks; observed above 50 FPS

##### Try to connect to the server from another computer.
Status: Networking is in place, but a real second-computer validation remains pending

##### Connect simultaneously with as many people as possible and play the game for at least 3 minutes.
Status: Local two-client startup is verified; three-minute sustained play remains pending

###### Did the frame rate stay over 50 fps?
Status: Not yet measured during the required three-minute multiplayer run

###### Independently of the frame rate displayed on the screen, does the game feel smooth?
Status: Movement and server tick handling are implemented; sustained multiplayer feel remains to be validated

### Bonus

###### +Is it possible to edit your own maze?
Status: Not yet implemented

###### +Are levels created automatically by an algorithm?
Status: Not yet implemented

###### +Can you play against an A.I. player?
Status: Not yet implemented

###### +Does the game initialization include a history of hosts with aliases for easier reconnection?
Status: Not yet implemented

## Required next milestone to pass the full audit

Remaining audit validation focuses on:

1. Remote client connection from another computer,
2. Three-minute multiplayer session with as many clients as practical,
3. Sustained multiplayer FPS and smoothness measurement.

Until those features exist, the audit cannot be marked as fully complete.
