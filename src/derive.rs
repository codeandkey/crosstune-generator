use std::collections::{HashMap, HashSet};

use anyhow::Result;

use crate::input::{CustomClue, PuzzleInput, Track};

const MIN_FILL_LEN: usize = 3;
const MAX_FILL_LEN: usize = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceKind {
    Title,
    Artist,
    Album,
    Snippet,
    Custom,
}

#[derive(Debug, Clone)]
pub struct CandidateAnswer {
    pub normalized: String,
    pub original_phrase: String,
    pub source_text: String,
    pub source_kind: SourceKind,
    pub track_index: usize,
    pub track_name: String,
    pub track_artist: String,
    pub span: std::ops::Range<usize>,
    pub quality_score: i32,
    pub custom_clue: Option<String>,
    pub ambiguity_count: usize,
}

pub fn derive_candidates(input: &PuzzleInput) -> Result<Vec<CandidateAnswer>> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let max_len = MAX_FILL_LEN.min(input.width.max(input.height));

    for (track_index, track) in input.tracks.iter().enumerate() {
        for custom in &track.custom {
            add_custom_candidate(&mut out, &mut seen, track_index, track, custom, max_len);
        }
        add_source_candidates(
            &mut out,
            &mut seen,
            track_index,
            track,
            SourceKind::Title,
            &track.name,
            max_len,
        );
        add_source_candidates(
            &mut out,
            &mut seen,
            track_index,
            track,
            SourceKind::Artist,
            &track.artist,
            max_len,
        );
        add_source_candidates(
            &mut out,
            &mut seen,
            track_index,
            track,
            SourceKind::Album,
            &track.album,
            max_len,
        );
        for snippet in &track.snippets {
            add_source_candidates(
                &mut out,
                &mut seen,
                track_index,
                track,
                SourceKind::Snippet,
                snippet,
                max_len,
            );
        }
    }

    apply_custom_overrides(&mut out, input);
    apply_priority_adjustments(&mut out);

    out.sort_by(|left, right| {
        right
            .quality_score
            .cmp(&left.quality_score)
            .then_with(|| right.normalized.len().cmp(&left.normalized.len()))
            .then_with(|| left.normalized.cmp(&right.normalized))
    });

    Ok(out)
}

fn apply_custom_overrides(candidates: &mut [CandidateAnswer], input: &PuzzleInput) {
    let custom_by_track_and_answer = input
        .tracks
        .iter()
        .enumerate()
        .flat_map(|(track_index, track)| {
            track.custom.iter().map(move |custom| {
                (
                    (track_index, normalize_for_grid(&custom.answer)),
                    custom.clue.clone(),
                )
            })
        })
        .collect::<HashMap<_, _>>();

    for candidate in candidates.iter_mut() {
        if let Some(clue) = custom_by_track_and_answer
            .get(&(candidate.track_index, candidate.normalized.clone()))
        {
            candidate.source_kind = SourceKind::Custom;
            candidate.custom_clue = Some(clue.clone());
            candidate.original_phrase = candidate.original_phrase.clone();
        }
    }
}

fn apply_priority_adjustments(candidates: &mut [CandidateAnswer]) {
    let total_tracks = candidates
        .iter()
        .map(|candidate| candidate.track_index)
        .collect::<HashSet<_>>()
        .len();
    let mut applicability = HashMap::<String, HashSet<usize>>::new();
    for candidate in candidates.iter() {
        applicability
            .entry(candidate.normalized.clone())
            .or_default()
            .insert(candidate.track_index);
    }

    for candidate in candidates.iter_mut() {
        let ambiguity_count = applicability
            .get(&candidate.normalized)
            .map(|tracks| tracks.len())
            .unwrap_or(1);
        candidate.ambiguity_count = ambiguity_count;
        candidate.quality_score += source_priority_bonus(candidate.source_kind);
        candidate.quality_score -= ambiguity_penalty(candidate.source_kind, ambiguity_count, total_tracks);
    }
}

fn add_custom_candidate(
    out: &mut Vec<CandidateAnswer>,
    seen: &mut HashSet<(usize, String)>,
    track_index: usize,
    track: &Track,
    custom: &CustomClue,
    max_len: usize,
) {
    let normalized = normalize_for_grid(&custom.answer);
    let length = normalized.len();
    if !(MIN_FILL_LEN..=max_len).contains(&length) {
        return;
    }
    if !seen.insert((track_index, normalized.clone())) {
        return;
    }

    out.push(CandidateAnswer {
        normalized,
        original_phrase: custom.answer.clone(),
        source_text: custom.answer.clone(),
        source_kind: SourceKind::Custom,
        track_index,
        track_name: track.name.clone(),
        track_artist: track.artist.clone(),
        span: 0..custom.answer.len(),
        quality_score: score_phrase(SourceKind::Custom, &normalize_for_grid(&custom.answer), 1, 0, 0, 1) + 20,
        custom_clue: Some(custom.clue.clone()),
        ambiguity_count: 1,
    });
}

fn add_source_candidates(
    out: &mut Vec<CandidateAnswer>,
    seen: &mut HashSet<(usize, String)>,
    track_index: usize,
    track: &Track,
    source_kind: SourceKind,
    source_text: &str,
    max_len: usize,
) {
    let words = tokenize_words(source_text);
    for start in 0..words.len() {
        let mut phrase_words = Vec::new();
        let mut start_char = 0;
        let mut end_char;
        for end in start..words.len() {
            let word = &words[end];
            if phrase_words.is_empty() {
                start_char = word.start;
            }
            end_char = word.end;
            phrase_words.push(word.token);
            let phrase = phrase_words.join(" ");
            let normalized = normalize_for_grid(&phrase);
            let length = normalized.len();
            if length > max_len {
                break;
            }
            if length < MIN_FILL_LEN {
                continue;
            }
            if should_skip_fill_blank_candidate(source_kind, source_text, &phrase, &phrase_words) {
                continue;
            }
            let span = start_char..end_char;
            if should_skip_partial_metadata_candidate(source_kind, source_text, &phrase, &span) {
                continue;
            }
            if !seen.insert((track_index, normalized.clone())) {
                continue;
            }

            let quality_score = score_phrase(source_kind, &normalized, phrase_words.len(), start, end, words.len());
            out.push(CandidateAnswer {
                normalized,
                original_phrase: phrase,
                source_text: source_text.to_string(),
                source_kind,
                track_index,
                track_name: track.name.clone(),
                track_artist: track.artist.clone(),
                span,
                quality_score,
                custom_clue: None,
                ambiguity_count: 1,
            });
        }
    }
}

fn should_skip_fill_blank_candidate(
    source_kind: SourceKind,
    source_text: &str,
    phrase: &str,
    phrase_words: &[&str],
) -> bool {
    if phrase == source_text {
        return false;
    }

    match source_kind {
        SourceKind::Title | SourceKind::Artist | SourceKind::Album => phrase_words.len() > 1,
        SourceKind::Snippet => phrase_words
            .iter()
            .all(|word| is_filler_word(&word.to_ascii_lowercase())),
        SourceKind::Custom => false,
    }
}

fn should_skip_partial_metadata_candidate(
    source_kind: SourceKind,
    source_text: &str,
    phrase: &str,
    span: &std::ops::Range<usize>,
) -> bool {
    if !matches!(source_kind, SourceKind::Title | SourceKind::Artist | SourceKind::Album) {
        return false;
    }
    if phrase == source_text {
        return false;
    }

    let prefix = &source_text[..span.start];
    let suffix = &source_text[span.end..];
    !has_informative_context(prefix, suffix)
}

#[derive(Debug)]
struct WordToken<'a> {
    token: &'a str,
    start: usize,
    end: usize,
}

fn tokenize_words(text: &str) -> Vec<WordToken<'_>> {
    let mut tokens = Vec::new();
    let mut current_start = None;
    for (index, ch) in text.char_indices() {
        let is_word_char = ch.is_alphanumeric()
            || ((ch == '\'' || ch == '’')
                && current_start.is_some()
                && text[index + ch.len_utf8()..]
                    .chars()
                    .next()
                    .is_some_and(|next| next.is_alphanumeric()));
        if is_word_char {
            current_start.get_or_insert(index);
        } else if let Some(start) = current_start.take() {
            tokens.push(WordToken {
                token: &text[start..index],
                start,
                end: index,
            });
        }
    }
    if let Some(start) = current_start {
        tokens.push(WordToken {
            token: &text[start..],
            start,
            end: text.len(),
        });
    }
    tokens
}

pub fn normalize_for_grid(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_alphabetic())
        .flat_map(|ch| ch.to_uppercase())
        .collect()
}

fn score_phrase(
    source_kind: SourceKind,
    normalized: &str,
    word_count: usize,
    start_word: usize,
    end_word: usize,
    total_words: usize,
) -> i32 {
    let mut score = normalized.len() as i32 * 10;
    score += normalized.chars().collect::<HashSet<_>>().len() as i32 * 2;
    if word_count == 1 {
        score += 8;
    } else {
        score += 4 - word_count as i32;
    }
    if start_word == 0 && end_word + 1 == total_words {
        score += 8;
    }
    score += match source_kind {
        SourceKind::Title => 12,
        SourceKind::Artist => 9,
        SourceKind::Album => 7,
        SourceKind::Snippet => 5,
        SourceKind::Custom => 18,
    };
    score
}

fn source_priority_bonus(source_kind: SourceKind) -> i32 {
    match source_kind {
        SourceKind::Custom => 120,
        SourceKind::Title => 30,
        SourceKind::Album => 6,
        SourceKind::Artist => -6,
        SourceKind::Snippet => -22,
    }
}

fn ambiguity_penalty(source_kind: SourceKind, ambiguity_count: usize, total_tracks: usize) -> i32 {
    if ambiguity_count <= 1 {
        return 0;
    }
    if total_tracks <= 2 {
        return match source_kind {
            SourceKind::Artist => 18,
            SourceKind::Album => 4,
            SourceKind::Snippet => 8,
            SourceKind::Title => 0,
            SourceKind::Custom => 0,
        };
    }

    let coverage = ambiguity_count as f32 / total_tracks as f32;
    if coverage < 0.6 {
        return 0;
    }

    let severity: f32 = if coverage >= 0.95 {
        1.0
    } else if coverage >= 0.8 {
        0.6
    } else {
        0.25
    };
    let base: f32 = match source_kind {
        SourceKind::Artist => 60.0,
        SourceKind::Album => 18.0,
        SourceKind::Snippet => 28.0,
        SourceKind::Title => 8.0,
        SourceKind::Custom => 4.0,
    };
    (base * severity).round() as i32
}

impl CandidateAnswer {
    pub fn clue_text(&self) -> String {
        if let Some(clue) = &self.custom_clue {
            return clue.clone();
        }

        match self.source_kind {
            SourceKind::Title => self.blank_clue("Title of this song", "Part of this song title"),
            SourceKind::Artist => self.blank_clue("Song artist", "Part of the artist name"),
            SourceKind::Album => self.blank_clue("Album containing this song", "Part of the album title"),
            SourceKind::Snippet => self.blank_clue("Lyric from this song", "Part of this lyric"),
            SourceKind::Custom => "Custom clue".to_string(),
        }
    }

    pub fn is_metadata_fill_in_blank(&self) -> bool {
        matches!(self.source_kind, SourceKind::Title | SourceKind::Artist | SourceKind::Album)
            && self.original_phrase != self.source_text
            && has_informative_context(&self.source_text[..self.span.start], &self.source_text[self.span.end..])
    }

    fn blank_clue(&self, whole_fallback: &str, partial_fallback: &str) -> String {
        if self.original_phrase == self.source_text {
            return whole_fallback.to_string();
        }

        let prefix = &self.source_text[..self.span.start];
        let suffix = &self.source_text[self.span.end..];
        if !has_informative_context(prefix, suffix) {
            return partial_fallback.to_string();
        }

        let mut pattern = String::new();
        pattern.push_str(prefix.trim_end());
        if !pattern.is_empty() {
            pattern.push(' ');
        }
        pattern.push_str(&blank_placeholder(&self.source_text[self.span.clone()]));
        let trimmed_suffix = suffix.trim_start();
        if !trimmed_suffix.is_empty() {
            if starts_with_attached_punctuation(trimmed_suffix) {
                pattern.push_str(trimmed_suffix);
            } else {
                pattern.push(' ');
                pattern.push_str(trimmed_suffix);
            }
        }

        match self.source_kind {
            SourceKind::Title => format!("Fill in the song title: {pattern}"),
            SourceKind::Artist => format!("Fill in the artist name: {pattern}"),
            SourceKind::Album => format!("Fill in the album name: {pattern}"),
            SourceKind::Snippet => format!("Fill in the lyric: {pattern}"),
            SourceKind::Custom => format!("Fill in the custom answer: {pattern}"),
        }
    }
}

fn blank_placeholder(text: &str) -> String {
    let mut out = String::new();
    let mut in_word = false;

    for ch in text.chars() {
        if ch.is_alphanumeric() {
            if !in_word {
                out.push_str("____");
                in_word = true;
            }
            continue;
        }

        if ch == '\'' || ch == '’' {
            continue;
        }

        in_word = false;
        out.push(ch);
    }

    out
}

fn starts_with_attached_punctuation(text: &str) -> bool {
    matches!(
        text.chars().next(),
        Some(',') | Some('.') | Some('!') | Some('?') | Some(';') | Some(':')
    )
}

fn has_informative_context(prefix: &str, suffix: &str) -> bool {
    let context = format!("{prefix} {suffix}");
    tokenize_words(&context).into_iter().any(|word| {
        let lower = word.token.to_ascii_lowercase();
        !is_filler_word(&lower)
    })
}

fn is_filler_word(word: &str) -> bool {
    matches!(
        word,
        "a"
            | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "but"
            | "by"
            | "did"
            | "do"
            | "does"
            | "for"
            | "from"
            | "had"
            | "has"
            | "have"
            | "he"
            | "her"
            | "him"
            | "his"
            | "i"
            | "if"
            | "in"
            | "into"
            | "is"
            | "it"
            | "its"
            | "me"
            | "my"
            | "of"
            | "oh"
            | "on"
            | "or"
            | "our"
            | "out"
            | "she"
            | "so"
            | "than"
            | "that"
            | "the"
            | "their"
            | "them"
            | "they"
            | "this"
            | "to"
            | "up"
            | "us"
            | "was"
            | "we"
            | "were"
            | "what"
            | "when"
            | "who"
            | "with"
            | "you"
            | "your"
    )
}

#[cfg(test)]
mod tests {
    use crate::input::{PuzzleInput, Track};

    use super::{SourceKind, derive_candidates, normalize_for_grid, score_phrase};

    #[test]
    fn normalizes_to_uppercase_letters() {
        assert_eq!(normalize_for_grid("d'Amour 123"), "DAMOUR");
    }

    #[test]
    fn prefers_title_phrases() {
        assert!(score_phrase(SourceKind::Title, "ROXANNE", 1, 0, 0, 1)
            > score_phrase(SourceKind::Snippet, "ROXANNE", 1, 0, 0, 1));
    }

    #[test]
    fn does_not_split_contractions_into_partial_words() {
        let input = PuzzleInput {
            width: 15,
            height: 15,
            tracks: vec![Track {
                name: "Roxanne".to_string(),
                artist: "The Police".to_string(),
                album: "Outlandos d'Amour".to_string(),
                snippets: vec!["You don't have to put on the red light".to_string()],
                custom: Vec::new(),
            }],
        };

        let candidates = derive_candidates(&input).unwrap();
        assert!(candidates.iter().any(|candidate| candidate.normalized == "DONTHAVETOPUTON"));
        assert!(!candidates.iter().any(|candidate| candidate.normalized == "YOU"));
        assert!(!candidates.iter().any(|candidate| candidate.normalized == "YOURENOT"));
        assert!(!candidates.iter().any(|candidate| candidate.normalized == "THAVETOPUTON"));
    }

    #[test]
    fn includes_custom_answer_and_clue() {
        let input = PuzzleInput {
            width: 12,
            height: 12,
            tracks: vec![Track {
                name: "The Modern Age".to_string(),
                artist: "The Strokes".to_string(),
                album: "Is This It".to_string(),
                snippets: vec![],
                custom: vec![crate::input::CustomClue {
                    answer: "Modern".to_string(),
                    clue: "Second word of this song title".to_string(),
                }],
            }],
        };

        let candidates = derive_candidates(&input).unwrap();
        let candidate = candidates
            .iter()
            .find(|candidate| candidate.normalized == "MODERN")
            .unwrap();
        assert!(matches!(candidate.source_kind, SourceKind::Custom));
        assert_eq!(candidate.clue_text(), "Second word of this song title");
    }

    #[test]
    fn custom_answers_get_maximum_priority() {
        let input = PuzzleInput {
            width: 12,
            height: 12,
            tracks: vec![Track {
                name: "The Modern Age".to_string(),
                artist: "The Strokes".to_string(),
                album: "Is This It".to_string(),
                snippets: vec![],
                custom: vec![crate::input::CustomClue {
                    answer: "Modern".to_string(),
                    clue: "Second word of this song title".to_string(),
                }],
            }],
        };

        let candidates = derive_candidates(&input).unwrap();
        let custom = candidates
            .iter()
            .find(|candidate| candidate.normalized == "MODERN")
            .unwrap();
        let title = candidates
            .iter()
            .find(|candidate| candidate.normalized == "THEMODERNAGE")
            .unwrap();

        assert!(matches!(custom.source_kind, SourceKind::Custom));
        assert!(custom.quality_score > title.quality_score);
    }

    #[test]
    fn metadata_partial_phrases_get_fill_in_the_blank_clues() {
        let input = PuzzleInput {
            width: 12,
            height: 12,
            tracks: vec![Track {
                name: "The Modern Age".to_string(),
                artist: "The Strokes".to_string(),
                album: "First Impressions of Earth".to_string(),
                snippets: vec![],
                custom: Vec::new(),
            }],
        };

        let candidates = derive_candidates(&input).unwrap();
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.normalized == "MODERNAGE"));
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.normalized == "STROKES"));

        let album = candidates
            .iter()
            .find(|candidate| candidate.normalized == "IMPRESSIONS")
            .unwrap();
        assert_eq!(album.clue_text(), "Fill in the album name: First ____ of Earth");
    }

    #[test]
    fn metadata_blanks_are_single_token_but_snippets_can_be_multi_token() {
        let input = PuzzleInput {
            width: 18,
            height: 18,
            tracks: vec![Track {
                name: "The Adults Are Talking".to_string(),
                artist: "The Strokes".to_string(),
                album: "First Impressions of Earth".to_string(),
                snippets: vec!["You don't have to put on the red light".to_string()],
                custom: Vec::new(),
            }],
        };

        let candidates = derive_candidates(&input).unwrap();
        assert!(candidates.iter().any(|candidate| candidate.normalized == "ADULTS"));
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.normalized == "ADULTSARE"));
        assert!(candidates.iter().any(|candidate| candidate.normalized == "DONTHAVE"));
        assert!(!candidates.iter().any(|candidate| candidate.normalized == "YOU"));
    }

    #[test]
    fn multi_token_blank_preserves_spacing_and_punctuation() {
        let input = PuzzleInput {
            width: 24,
            height: 24,
            tracks: vec![Track {
                name: "The Modern Age".to_string(),
                artist: "The Strokes".to_string(),
                album: "Is This It".to_string(),
                snippets: vec!["No time to feel the breeze, I took too many varieties".to_string()],
                custom: Vec::new(),
            }],
        };

        let candidates = derive_candidates(&input).unwrap();
        let candidate = candidates
            .iter()
            .find(|candidate| candidate.normalized == "FEELTHEBREEZE")
            .unwrap();
        assert_eq!(
            candidate.clue_text(),
            "Fill in the lyric: No time to ____ ____ ____, I took too many varieties"
        );
    }

    #[test]
    fn partial_title_never_uses_full_title_clue() {
        let input = PuzzleInput {
            width: 16,
            height: 12,
            tracks: vec![Track {
                name: "You Only Live Once".to_string(),
                artist: "The Strokes".to_string(),
                album: "First Impressions of Earth".to_string(),
                snippets: vec![],
                custom: Vec::new(),
            }],
        };

        let candidates = derive_candidates(&input).unwrap();
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.normalized == "ONLYLIVEONCE"));
    }

    #[test]
    fn title_candidates_are_ranked_above_shared_artist_candidates() {
        let input = PuzzleInput {
            width: 12,
            height: 12,
            tracks: vec![
                Track {
                    name: "Reptilia".to_string(),
                    artist: "The Strokes".to_string(),
                    album: "Room On Fire".to_string(),
                    snippets: vec![],
                    custom: Vec::new(),
                },
                Track {
                    name: "Someday".to_string(),
                    artist: "The Strokes".to_string(),
                    album: "Is This It".to_string(),
                    snippets: vec![],
                    custom: Vec::new(),
                },
            ],
        };

        let candidates = derive_candidates(&input).unwrap();
        let title = candidates
            .iter()
            .find(|candidate| candidate.track_index == 0 && candidate.normalized == "REPTILIA")
            .unwrap();
        let artist = candidates
            .iter()
            .find(|candidate| candidate.track_index == 0 && candidate.normalized == "THESTROKES")
            .unwrap();

        assert_eq!(artist.ambiguity_count, 2);
        assert!(title.quality_score > artist.quality_score);
    }

    #[test]
    fn subset_album_overlap_is_not_heavily_penalized() {
        let input = PuzzleInput {
            width: 12,
            height: 12,
            tracks: vec![
                Track {
                    name: "Track One".to_string(),
                    artist: "Artist A".to_string(),
                    album: "Shared Album".to_string(),
                    snippets: vec![],
                    custom: Vec::new(),
                },
                Track {
                    name: "Track Two".to_string(),
                    artist: "Artist B".to_string(),
                    album: "Shared Album".to_string(),
                    snippets: vec![],
                    custom: Vec::new(),
                },
                Track {
                    name: "Track Three".to_string(),
                    artist: "Artist C".to_string(),
                    album: "Different Album".to_string(),
                    snippets: vec![],
                    custom: Vec::new(),
                },
                Track {
                    name: "Track Four".to_string(),
                    artist: "Artist D".to_string(),
                    album: "Another Album".to_string(),
                    snippets: vec![],
                    custom: Vec::new(),
                },
            ],
        };

        let candidates = derive_candidates(&input).unwrap();
        let shared_album = candidates
            .iter()
            .find(|candidate| candidate.track_index == 0 && candidate.normalized == "SHAREDALBUM")
            .unwrap();
        assert_eq!(shared_album.ambiguity_count, 2);
        assert!(shared_album.quality_score > 0);
    }

    #[test]
    fn near_global_artist_overlap_is_penalized() {
        let input = PuzzleInput {
            width: 12,
            height: 12,
            tracks: vec![
                Track {
                    name: "Track One".to_string(),
                    artist: "The Strokes".to_string(),
                    album: "Album One".to_string(),
                    snippets: vec![],
                    custom: Vec::new(),
                },
                Track {
                    name: "Track Two".to_string(),
                    artist: "The Strokes".to_string(),
                    album: "Album Two".to_string(),
                    snippets: vec![],
                    custom: Vec::new(),
                },
                Track {
                    name: "Track Three".to_string(),
                    artist: "The Strokes".to_string(),
                    album: "Album Three".to_string(),
                    snippets: vec![],
                    custom: Vec::new(),
                },
                Track {
                    name: "Track Four".to_string(),
                    artist: "The Strokes".to_string(),
                    album: "Album Four".to_string(),
                    snippets: vec![],
                    custom: Vec::new(),
                },
            ],
        };

        let candidates = derive_candidates(&input).unwrap();
        let artist = candidates
            .iter()
            .find(|candidate| candidate.track_index == 0 && candidate.normalized == "THESTROKES")
            .unwrap();
        let title = candidates
            .iter()
            .find(|candidate| candidate.track_index == 0 && candidate.normalized == "TRACKONE")
            .unwrap();

        assert_eq!(artist.ambiguity_count, 4);
        assert!(artist.quality_score < title.quality_score);
    }

}
