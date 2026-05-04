use anyhow::{Result, bail};
use serde::Deserialize;

pub const DEFAULT_MAX_TOKENS: usize = 3;

#[derive(Debug, Clone, Deserialize)]
pub struct PuzzleInput {
    pub width: usize,
    pub height: usize,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Track {
    pub name: String,
    pub artist: String,
    pub album: Option<String>,
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

        if self.max_tokens == 0 {
            bail!("max_tokens must be positive");
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
            if let Some(album) = &track.album
                && album.trim().is_empty()
            {
                bail!("{label} contains an empty album");
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

fn default_max_tokens() -> usize {
    DEFAULT_MAX_TOKENS
}

#[cfg(test)]
mod tests {
    use super::PuzzleInput;

    #[test]
    fn allows_null_album_in_yaml_input() {
        let yaml = r#"
width: 5
height: 5
tracks:
  - name: Reptilia
    artist: The Strokes
    album: null
"#;

        let input: PuzzleInput = serde_yaml::from_str(yaml).unwrap();
        input.validate().unwrap();
        assert!(input.tracks[0].album.is_none());
    }

    #[test]
    fn defaults_max_tokens_to_three() {
        let yaml = r#"
width: 5
height: 5
tracks:
  - name: Reptilia
    artist: The Strokes
"#;

        let input: PuzzleInput = serde_yaml::from_str(yaml).unwrap();
        input.validate().unwrap();
        assert_eq!(input.max_tokens, 3);
    }
}
