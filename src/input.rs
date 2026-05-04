use anyhow::{Result, bail};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct PuzzleInput {
    pub width: usize,
    pub height: usize,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Track {
    pub name: String,
    pub artist: String,
    pub album: String,
    #[serde(default)]
    pub snippets: Vec<String>,
    #[serde(default)]
    pub custom: Vec<CustomClue>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CustomClue {
    pub answer: String,
    pub clue: String,
}

impl PuzzleInput {
    pub fn validate(&self) -> Result<()> {
        if self.width == 0 || self.height == 0 {
            bail!("width and height must both be positive");
        }

        if self.tracks.is_empty() {
            bail!("at least one track is required");
        }

        for (index, track) in self.tracks.iter().enumerate() {
            let label = format!("track #{}", index + 1);
            if track.name.trim().is_empty() {
                bail!("{label} is missing a name");
            }
            if track.artist.trim().is_empty() {
                bail!("{label} is missing an artist");
            }
            if track.album.trim().is_empty() {
                bail!("{label} is missing an album");
            }
            for snippet in &track.snippets {
                if snippet.trim().is_empty() {
                    bail!("{label} contains an empty snippet");
                }
            }
            for custom in &track.custom {
                if custom.answer.trim().is_empty() {
                    bail!("{label} contains a custom clue with an empty answer");
                }
                if custom.clue.trim().is_empty() {
                    bail!("{label} contains a custom clue with an empty clue");
                }
            }
        }

        Ok(())
    }
}
