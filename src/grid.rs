use std::collections::HashMap;

use crate::derive::CandidateAnswer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Across,
    Down,
}

#[derive(Debug, Clone)]
pub struct PlacedEntry {
    pub candidate: CandidateAnswer,
    pub x: usize,
    pub y: usize,
    pub direction: Direction,
    pub crossings: usize,
}

#[derive(Debug, Clone)]
pub struct Grid {
    pub width: usize,
    pub height: usize,
    cells: Vec<Option<char>>,
    pub entries: Vec<PlacedEntry>,
}

#[derive(Debug, Clone)]
pub struct NumberedEntry {
    pub number: usize,
    pub x: usize,
    pub y: usize,
    pub direction: Direction,
    pub clue: String,
    pub answer: String,
    pub source: String,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![None; width * height],
            entries: Vec::new(),
        }
    }

    pub fn cell(&self, x: usize, y: usize) -> Option<char> {
        self.cells[self.index(x, y)]
    }

    pub fn filled_count(&self) -> usize {
        self.cells.iter().filter(|cell| cell.is_some()).count()
    }

    pub fn place(&self, candidate: &CandidateAnswer, x: usize, y: usize, direction: Direction) -> Option<Self> {
        let mut next = self.clone();
        let crossings = next.can_place(candidate, x, y, direction)?;

        for (step, ch) in candidate.normalized.chars().enumerate() {
            let (cx, cy) = advance(x, y, direction, step);
            let idx = next.index(cx, cy);
            next.cells[idx] = Some(ch);
        }
        next.entries.push(PlacedEntry {
            candidate: candidate.clone(),
            x,
            y,
            direction,
            crossings,
        });
        Some(next)
    }

    pub fn placements_for(&self, candidate: &CandidateAnswer) -> Vec<(usize, usize, Direction, usize)> {
        let mut out = Vec::new();
        for direction in [Direction::Across, Direction::Down] {
            if (direction == Direction::Across && candidate.normalized.len() > self.width)
                || (direction == Direction::Down && candidate.normalized.len() > self.height)
            {
                continue;
            }
            let max_x = if direction == Direction::Across {
                self.width.saturating_sub(candidate.normalized.len())
            } else {
                self.width.saturating_sub(1)
            };
            let max_y = if direction == Direction::Down {
                self.height.saturating_sub(candidate.normalized.len())
            } else {
                self.height.saturating_sub(1)
            };
            for y in 0..=max_y {
                for x in 0..=max_x {
                    if let Some(crossings) = self.can_place(candidate, x, y, direction) {
                        out.push((x, y, direction, crossings));
                    }
                }
            }
        }
        out
    }

    pub fn numbered_entries(&self) -> Vec<NumberedEntry> {
        let mut starts = HashMap::new();
        let mut next_number = 1;
        let mut sorted = self.entries.clone();
        sorted.sort_by_key(|entry| (entry.y, entry.x, matches!(entry.direction, Direction::Down)));
        for entry in &sorted {
            starts.entry((entry.x, entry.y)).or_insert_with(|| {
                let current = next_number;
                next_number += 1;
                current
            });
        }

        sorted
            .into_iter()
            .map(|entry| NumberedEntry {
                number: starts[&(entry.x, entry.y)],
                x: entry.x,
                y: entry.y,
                direction: entry.direction,
                clue: entry.candidate.clue_text(),
                answer: entry.candidate.normalized.clone(),
                source: format!("{} - {}", entry.candidate.track_name, entry.candidate.track_artist),
            })
            .collect()
    }

    fn can_place(&self, candidate: &CandidateAnswer, x: usize, y: usize, direction: Direction) -> Option<usize> {
        let len = candidate.normalized.len();
        if len == 0 {
            return None;
        }
        match direction {
            Direction::Across if x + len > self.width => return None,
            Direction::Down if y + len > self.height => return None,
            _ => {}
        }

        if self.has_entry_conflict(candidate, x, y, direction) {
            return None;
        }

        if self.before_or_after_filled(x, y, direction, len) {
            return None;
        }

        let mut crossings = 0;
        let has_existing = !self.entries.is_empty();

        for (step, ch) in candidate.normalized.chars().enumerate() {
            let (cx, cy) = advance(x, y, direction, step);
            match self.cell(cx, cy) {
                Some(existing) if existing != ch => return None,
                Some(_) => {
                    crossings += 1;
                }
                None => {
                    if self.side_touching(cx, cy, direction) {
                        return None;
                    }
                }
            }
        }

        if has_existing && crossings == 0 {
            return None;
        }

        Some(crossings)
    }

    fn before_or_after_filled(&self, x: usize, y: usize, direction: Direction, len: usize) -> bool {
        let before = match direction {
            Direction::Across => x.checked_sub(1).map(|bx| (bx, y)),
            Direction::Down => y.checked_sub(1).map(|by| (x, by)),
        };
        let after = match direction {
            Direction::Across => Some((x + len, y)).filter(|(ax, _)| *ax < self.width),
            Direction::Down => Some((x, y + len)).filter(|(_, ay)| *ay < self.height),
        };
        before.into_iter().chain(after).any(|(cx, cy)| self.cell(cx, cy).is_some())
    }

    fn side_touching(&self, x: usize, y: usize, direction: Direction) -> bool {
        let neighbors = match direction {
            Direction::Across => [offset(x, y, 0, -1), offset(x, y, 0, 1)],
            Direction::Down => [offset(x, y, -1, 0), offset(x, y, 1, 0)],
        };
        neighbors
            .into_iter()
            .flatten()
            .filter(|(nx, ny)| *nx < self.width && *ny < self.height)
            .any(|(nx, ny)| self.cell(nx, ny).is_some())
    }

    fn has_entry_conflict(&self, candidate: &CandidateAnswer, x: usize, y: usize, direction: Direction) -> bool {
        self.entries.iter().any(|entry| {
            entry.candidate.track_index == candidate.track_index
                || entry.candidate.normalized == candidate.normalized
                || (entry.x == x
                    && entry.y == y
                    && entry.direction == direction
                    && entry.candidate.normalized == candidate.normalized)
        })
    }

    fn index(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }
}

fn advance(x: usize, y: usize, direction: Direction, step: usize) -> (usize, usize) {
    match direction {
        Direction::Across => (x + step, y),
        Direction::Down => (x, y + step),
    }
}

fn offset(x: usize, y: usize, dx: isize, dy: isize) -> Option<(usize, usize)> {
    let nx = x.checked_add_signed(dx)?;
    let ny = y.checked_add_signed(dy)?;
    Some((nx, ny))
}

#[cfg(test)]
mod tests {
    use crate::derive::{CandidateAnswer, SourceKind};

    use super::{Direction, Grid};

    fn candidate(track_index: usize, normalized: &str) -> CandidateAnswer {
        CandidateAnswer {
            normalized: normalized.to_string(),
            original_phrase: normalized.to_string(),
            source_text: normalized.to_string(),
            source_kind: SourceKind::Title,
            track_index,
            track_name: format!("Track {track_index}"),
            track_artist: "Artist".to_string(),
            span: 0..normalized.len(),
            quality_score: 0,
            custom_clue: None,
            ambiguity_count: 1,
        }
    }

    #[test]
    fn rejects_non_crossing_second_entry() {
        let grid = Grid::new(5, 5)
            .place(&candidate(0, "ROX"), 0, 0, Direction::Across)
            .unwrap();
        assert!(grid.place(&candidate(1, "ANN"), 0, 2, Direction::Across).is_none());
    }
}
