pub struct Player {
	pub name: String,
	pub x: f32,
	pub y: f32,
}

pub struct Maze {
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<Vec<char>>,
}

impl Player {
    pub fn new(name: String, x: f32, y: f32) -> Player {
        Player { name, x, y }
    }
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
    use super::Maze;

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
}
