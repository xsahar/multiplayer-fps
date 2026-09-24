#[test]
fn collision_prevents_wall_entry() {
    let map = vec![
        vec![1, 1, 1, 1, 1],
        vec![1, 0, 0, 0, 1],
        vec![1, 0, 1, 0, 1],
        vec![1, 0, 0, 0, 1],
        vec![1, 1, 1, 1, 1],
    ];

    let position = (2.0, 2.0);
    let next = (2.0, 1.5);
    let result = maze_wars::shared::collision::move_with_collision(position, next, &map, 0.4);

    assert!(result.1 >= 1.5);
    assert!(result.0 >= 1.0);
}

#[test]
fn protocol_round_trip_serializes() {
    let packet = maze_wars::shared::protocol::ClientMessage::Hello {
        username: "alice".to_string(),
        protocol_version: 1,
    };

    let encoded = bincode::serialize(&packet).unwrap();
    let decoded: maze_wars::shared::protocol::ClientMessage = bincode::deserialize(&encoded).unwrap();

    match decoded {
        maze_wars::shared::protocol::ClientMessage::Hello {
            username,
            protocol_version,
        } => {
            assert_eq!(username, "alice");
            assert_eq!(protocol_version, 1);
        }
        _ => panic!("wrong message variant"),
    }
}
