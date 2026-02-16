use embedded_graphics::prelude::Point;

use crate::assets::{LIKE_A_STONE_MUSIC, TETRIS_MUSIC};

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Musics {
    LikeStone,
    Tetris,
}
impl Musics {
    const ALL: [Musics; 2] = [Musics::Tetris, Musics::LikeStone];
    pub fn next(&self) -> Self {
        let index = Self::ALL.iter().position(|x| x == self).unwrap();
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn prev(&self) -> Self {
        let index = Self::ALL.iter().position(|x| x == self).unwrap();
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub fn bytes(&self) -> &'static [u8] {
        match self {
            Musics::LikeStone => LIKE_A_STONE_MUSIC,
            Musics::Tetris => TETRIS_MUSIC,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Musics::LikeStone => "Like a Stone",
            Musics::Tetris => "Tetris",
        }
    }

    pub fn title_pos(&self) -> Point {
        match self {
            Musics::LikeStone => Point::new(17, 15),
            Musics::Tetris => Point::new(35, 15),
        }
    }

    pub fn from_index(idx: &u8) -> Self {
        match idx {
            0 => Musics::Tetris,
            1 => Musics::LikeStone,
            _ => Musics::Tetris,
        }
    }

    pub fn to_index(&self) -> u8 {
        match self {
            Musics::Tetris => 0,
            Musics::LikeStone => 1,
        }
    }
}
