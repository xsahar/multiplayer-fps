pub fn move_with_collision(
    position: (f32, f32),
    target: (f32, f32),
    map: &[Vec<u8>],
    radius: f32,
) -> (f32, f32) {
    let width = map[0].len() as f32;
    let height = map.len() as f32;
    let can_stand_at = |candidate: (f32, f32)| {
        let x = candidate.0.clamp(radius, width - radius);
        let y = candidate.1.clamp(radius, height - radius);
        let min_x = (x - radius).floor().max(0.0) as usize;
        let max_x = (x + radius).floor().min(width - 1.0) as usize;
        let min_y = (y - radius).floor().max(0.0) as usize;
        let max_y = (y + radius).floor().min(height - 1.0) as usize;

        for cell_y in min_y..=max_y {
            for cell_x in min_x..=max_x {
                if map[cell_y][cell_x] != 1 {
                    continue;
                }

                let nearest_x = x.clamp(cell_x as f32, cell_x as f32 + 1.0);
                let nearest_y = y.clamp(cell_y as f32, cell_y as f32 + 1.0);
                let distance_x = x - nearest_x;
                let distance_y = y - nearest_y;
                if distance_x * distance_x + distance_y * distance_y < radius * radius {
                    return false;
                }
            }
        }

        true
    };

    let clamped_target = (
        target.0.clamp(radius, width - radius),
        target.1.clamp(radius, height - radius),
    );

    // Resolve each axis independently so diagonal movement slides along walls.
    let after_x = if can_stand_at((clamped_target.0, position.1)) {
        (clamped_target.0, position.1)
    } else {
        position
    };
    if can_stand_at((after_x.0, clamped_target.1)) {
        (after_x.0, clamped_target.1)
    } else {
        after_x
    }
}
