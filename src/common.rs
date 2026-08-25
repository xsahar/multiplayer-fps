pub const MAX_HEALTH: u8 = 100;
pub const HIT_DAMAGE: u8 = 10;
pub const PLAYER_HIT_RADIUS: f32 = 0.5;
pub const RAY_STEP: f32 = 0.02;
pub const RAY_MAX: f32 = 20.0;

pub struct Player {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub health: u8,
    pub alive: bool,
    pub kills: u32,
    pub deaths: u32,
}

pub struct Maze {
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<Vec<char>>,
}

impl Player {
    pub fn new(name: String, x: f32, y: f32) -> Player {
        Player {
            name,
            x,
            y,
            health: MAX_HEALTH,
            alive: true,
            kills: 0,
            deaths: 0,
        }
    }

    /// Applies one hit. Returns true if this hit killed the player.
    pub fn apply_hit(&mut self) -> bool {
        if !self.alive {
            return false;
        }

        self.health = self.health.saturating_sub(HIT_DAMAGE);
        if self.health == 0 {
            self.alive = false;
            self.deaths += 1;
            true
        } else {
            false
        }
    }

    pub fn respawn_for_round(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
        self.health = MAX_HEALTH;
        self.alive = true;
    }

    pub fn record_kill(&mut self) {
        self.kills += 1;
    }

    pub fn reset_stats(&mut self) {
        self.kills = 0;
        self.deaths = 0;
    }
}

/// Marches a ray until a wall and returns the closest living target within hit radius.
pub fn raycast_to_player(
    maze: &Maze,
    start_x: f32,
    start_y: f32,
    angle: f32,
    targets: &[(u32, f32, f32)],
) -> Option<(u32, f32)> {
    let direction_x = angle.cos();
    let direction_y = angle.sin();
    let mut distance = 0.0;
    let radius_sq = PLAYER_HIT_RADIUS * PLAYER_HIT_RADIUS;

    while distance < RAY_MAX {
        let ray_x = start_x + direction_x * distance;
        let ray_y = start_y + direction_y * distance;
        let tile_x = ray_x.floor() as isize;
        let tile_y = ray_y.floor() as isize;

        if tile_x < 0 || tile_y < 0 || maze.is_wall(tile_x as usize, tile_y as usize) {
            return None;
        }

        for &(id, px, py) in targets {
            let dx = ray_x - px;
            let dy = ray_y - py;
            if dx * dx + dy * dy <= radius_sq {
                return Some((id, distance));
            }
        }

        distance += RAY_STEP;
    }

    None
}

impl Maze{
    pub fn get(&self, x: usize, y: usize) -> Option<char> {
        if x >= self.width || y >= self.height {
            return None;
        }

        Some(self.tiles[y][x])
    }

    pub fn set(&mut self, x: usize, y: usize, tile: char) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }

        self.tiles[y][x] = tile;
        true
    }

    pub fn is_wall(&self, x: usize, y: usize) -> bool {
      if x >= self.width || y >= self.height{
        return true;
      }

      self.tiles[y][x] == '#'
    }

    pub fn try_move(&self, player: &mut Player, next_x: usize, next_y: usize) -> bool{
        if self.is_wall(next_x, next_y){
            return false;
        }
        player.x = next_x as f32;
        player.y = next_y as f32;
        true
    }

    pub fn move_player(
        &self,
        player: &mut Player,
        change_x: isize,
        change_y: isize,
    ) -> bool {
        let next_x = player.x as isize + change_x;
        let next_y = player.y as isize + change_y;

        if next_x < 0 || next_y < 0 {
            return false;
        }
        self.try_move(player, next_x as usize, next_y as usize)
    }

    pub fn find_spawn(&self) -> Option<(usize, usize)> {
        for y in 0..self.height {
            for x in 0..self.width {
                if !self.is_wall(x, y) {
                    return Some((x, y));
                }
            }
        }

        None
    }

    pub fn simple() -> Maze {
     Maze {
        width: 5,
        height: 5,
        tiles: vec![
            vec!['#', '#', '#', '#', '#'],
        vec!['#', '.', '.', '.', '#'],
        vec!['#', '.', '#', '.', '#'],
        vec!['#', '.', '.', '.', '#'],
        vec!['#', '#', '#', '#', '#'],
        ],
    }
}

}

#[cfg(test)]
mod tests {
    use super::{raycast_to_player, Maze, Player};

    #[test]
    fn finds_floor_spawn() {
        let maze = Maze::simple();

        assert_eq!(maze.find_spawn(), Some((1, 1)));
    }

    #[test]
    fn reads_and_changes_tiles() {
        let mut maze = Maze::simple();

        assert_eq!(maze.get(2, 2), Some('#'));
        assert!(maze.set(2, 2, '.'));
        assert_eq!(maze.get(2, 2), Some('.'));
        assert_eq!(maze.get(5, 0), None);
        assert!(!maze.set(5, 0, '.'));
    }

    #[test]
    fn ray_hits_player_in_open_corridor() {
        let maze = Maze::simple();
        let targets = [(2, 3.5, 1.5)];

        let hit = raycast_to_player(&maze, 1.5, 1.5, 0.0, &targets);

        assert!(hit.is_some());
        let (id, distance) = hit.unwrap();
        assert_eq!(id, 2);
        assert!(distance < 2.5);
    }

    #[test]
    fn ray_misses_player_behind_wall() {
        let maze = Maze::simple();
        let targets = [(2, 3.5, 2.5)];

        let hit = raycast_to_player(&maze, 1.5, 2.5, 0.0, &targets);

        assert_eq!(hit, None);
    }

    #[test]
    fn ten_hits_kill_a_player() {
        let mut player = Player::new(String::from("target"), 1.5, 1.5);

        for expected_health in [90, 80, 70, 60, 50, 40, 30, 20, 10] {
            assert!(!player.apply_hit());
            assert_eq!(player.health, expected_health);
            assert!(player.alive);
        }

        assert!(player.apply_hit());
        assert_eq!(player.health, 0);
        assert!(!player.alive);
        assert_eq!(player.deaths, 1);
        assert!(!player.apply_hit());
    }
}
