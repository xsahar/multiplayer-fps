use rand::prelude::*;

#[derive(Clone)]
pub struct Maze {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<u8>, 
}

impl Maze {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            tiles: vec![1; (width * height) as usize],
        }
    }

    #[inline]
    pub fn idx(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }

    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 {
            return 1;
        }
        let (x, y) = (x as u32, y as u32);
        if x >= self.width || y >= self.height {
            return 1;
        }
        self.tiles[self.idx(x, y)]
    }

    pub fn set(&mut self, x: u32, y: u32, v: u8) {
        let i = self.idx(x, y);
        self.tiles[i] = v;
    }

    pub fn generate_prims(width: u32, height: u32, mut rng: impl Rng) -> Self {
        let mut mz = Maze::new(width | 1, height | 1);

        // start cell
        let sx = 1;
        let sy = 1;
        mz.set(sx, sy, 0);

        let mut frontier = vec![(sx, sy)];
        while let Some(&(cx, cy)) = frontier.choose(&mut rng) {
            let candidates = [
                (cx as i32 + 2, cy as i32, cx as i32 + 1, cy as i32),
                (cx as i32 - 2, cy as i32, cx as i32 - 1, cy as i32),
                (cx as i32, cy as i32 + 2, cx as i32, cy as i32 + 1),
                (cx as i32, cy as i32 - 2, cx as i32, cy as i32 - 1),
            ];

            let mut carved = false;
            for &(nx, ny, wx, wy) in candidates.choose_multiple(&mut rng, 4) {
                if nx <= 0 || ny <= 0 {
                    continue;
                }
                if (nx as u32) >= mz.width - 1 || (ny as u32) >= mz.height - 1 {
                    continue;
                }
                if mz.get(nx, ny) == 1 {
                    // carve corridor
                    mz.set(nx as u32, ny as u32, 0);
                    mz.set(wx as u32, wy as u32, 0);
                    frontier.push((nx as u32, ny as u32));
                    carved = true;
                    break;
                }
            }

            if !carved {
                if let Some(pos) = frontier.iter().position(|&p| p == (cx, cy)) {
                    frontier.swap_remove(pos);
                }
            }
        }

        // keep solid borders
        for x in 0..mz.width {
            mz.set(x, 0, 1);
            mz.set(x, mz.height - 1, 1);
        }
        for y in 0..mz.height {
            mz.set(0, y, 1);
            mz.set(mz.width - 1, y, 1);
        }

        mz
    }

    /// Knocks down some interior walls that sit directly between two already-carved
    /// cells, turning a "perfect" maze (one dead-end-heavy path between any two points)
    /// into one with loops and shortcuts. `chance` (0.0-1.0) is the probability any given
    /// removable wall gets opened; higher chance = fewer dead ends, easier to navigate.
    pub fn braid(&mut self, rng: &mut impl Rng, chance: f32) {
        if chance <= 0.0 {
            return;
        }
        for y in 1..(self.height - 1) {
            for x in 1..(self.width - 1) {
                if self.get(x as i32, y as i32) != 1 {
                    continue;
                }
                let n = self.get(x as i32, y as i32 - 1);
                let s = self.get(x as i32, y as i32 + 1);
                let e = self.get(x as i32 + 1, y as i32);
                let w = self.get(x as i32 - 1, y as i32);
                let connects_h = w == 0 && e == 0 && n == 1 && s == 1;
                let connects_v = n == 0 && s == 0 && e == 1 && w == 1;
                if (connects_h || connects_v) && rng.gen::<f32>() < chance {
                    self.set(x, y, 0);
                }
            }
        }
    }

    /// Pick a random floor tile as a spawn point; returns center-of-tile coords.
    pub fn random_spawn(&self, rng: &mut impl Rng) -> (f32, f32) {
        loop {
            let x = rng.gen_range(1..(self.width - 1));
            let y = rng.gen_range(1..(self.height - 1));
            if self.get(x as i32, y as i32) == 0 {
                return (x as f32 + 0.5, y as f32 + 0.5);
            }
        }
    }
}

pub fn level_dims(level: crate::common::protocol::LevelKind) -> (u32, u32) {
    use crate::common::protocol::LevelKind::*;
    match level {
        Easy => (23, 15),
        Normal => (39, 27),
        Hard => (55, 37),
    }
}
