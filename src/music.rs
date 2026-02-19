use embedded_graphics::prelude::Point;

use crate::assets;

/// Enum representing available tracks in the system.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Musics {
    LikeStone,
    Tetris,
    MarioWorld,
    TopGear,
}

impl Musics {
    /// Internal registry of all tracks to facilitate indexing and navigation.
    const ALL: [Musics; 4] = [
        Musics::Tetris,
        Musics::LikeStone,
        Musics::MarioWorld,
        Musics::TopGear,
    ];

    /// Returns the next track in the list, wrapping back to the start if at the end.
    pub fn next(&self) -> Self {
        let index = Self::ALL.iter().position(|x| x == self).unwrap();
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    /// Returns the previous track, wrapping to the end if at the start.
    pub fn prev(&self) -> Self {
        let index = Self::ALL.iter().position(|x| x == self).unwrap();
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    /// Returns a static reference to the raw audio bytes stored in Flash.
    pub fn bytes(&self) -> &'static [u8] {
        match self {
            Musics::LikeStone => assets::LIKE_A_STONE_MUSIC,
            Musics::Tetris => assets::TETRIS_MUSIC,
            Musics::MarioWorld => assets::MARIO_WORLD,
            Musics::TopGear => assets::TOP_GEAR,
        }
    }

    /// Returns the displayable string title for the track.
    pub fn title(&self) -> &'static str {
        match self {
            Musics::LikeStone => "Like a Stone",
            Musics::Tetris => "Tetris",
            Musics::MarioWorld => "Mario World",
            Musics::TopGear => "Top Gear",
        }
    }

    /// Provides the UI coordinates (X, Y) to render the title on the OLED.
    /// Values are manually tuned for centering based on title length.
    pub fn title_pos(&self) -> Point {
        match self {
            Musics::LikeStone => Point::new(17, 15),
            Musics::Tetris => Point::new(35, 15),
            Musics::MarioWorld => Point::new(19, 15),
            Musics::TopGear => Point::new(28, 15),
        }
    }

    /// Factory method to retrieve a track variant from a numeric index.
    pub fn from_index(idx: &u8) -> Self {
        match idx {
            0 => Musics::Tetris,
            1 => Musics::LikeStone,
            2 => Musics::MarioWorld,
            3 => Musics::TopGear,
            _ => Musics::Tetris, // Default fallback
        }
    }

    /// Converts the current track variant back into a numeric index.
    pub fn to_index(&self) -> u8 {
        match self {
            Musics::Tetris => 0,
            Musics::LikeStone => 1,
            Musics::MarioWorld => 2,
            Musics::TopGear => 3,
        }
    }
}
