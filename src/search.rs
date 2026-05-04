use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};

use rand::{SeedableRng, prelude::SliceRandom};
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;

use crate::{
    derive::{CandidateAnswer, SourceKind},
    grid::{Direction, Grid},
    input::PuzzleInput,
};

#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub time_limit: Duration,
    pub seed: u64,
    pub branch_limit: usize,
    pub top_n: usize,
}

#[derive(Debug, Clone)]
pub struct Solution {
    pub grid: Grid,
    pub score: Score,
    pub elapsed: Duration,
    pub explored_nodes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Score {
    pub filled_cells: usize,
    pub crossings: usize,
    pub hub_entries: usize,
    pub title_entries: usize,
    pub snippet_entries: usize,
    pub metadata_blank_entries: usize,
    pub clue_quality: i32,
}

struct SearchContext<'a> {
    candidates: &'a [CandidateAnswer],
    deadline: Instant,
    branch_limit: usize,
    top_n: usize,
}

pub fn search_best_n(input: &PuzzleInput, candidates: &[CandidateAnswer], config: &SearchConfig) -> Option<Vec<Solution>> {
    let start = Instant::now();
    let deadline = start + config.time_limit;
    let context = Arc::new(SearchContext {
        candidates,
        deadline,
        branch_limit: config.branch_limit.max(1),
        top_n: config.top_n.max(1),
    });
    let root_grid = Grid::new(input.width, input.height);

    let root_branches = initial_branches(&root_grid, candidates, config.branch_limit, config.seed);
    if root_branches.is_empty() {
        return None;
    }

    let results: Vec<_> = root_branches
        .into_par_iter()
        .enumerate()
        .map(|(index, branch)| {
            let mut rng = ChaCha8Rng::seed_from_u64(config.seed.wrapping_add(index as u64 + 1));
            let mut explored = 1;
            let best = dfs(branch, &context, 1, &mut explored, &mut rng);
            (best, explored)
        })
        .collect();

    let mut total_nodes = 0usize;
    let mut best_grids = Vec::new();
    let mut seen = HashSet::new();
    for (candidates, explored) in results {
        total_nodes += explored;
        for grid in candidates {
            insert_top_grid(&mut best_grids, &mut seen, grid, config.top_n.max(1));
        }
    }

    if best_grids.is_empty() {
        return None;
    }

    let elapsed = start.elapsed();
    Some(
        best_grids
            .into_iter()
            .map(|grid| Solution {
                score: score_grid(&grid),
                grid,
                elapsed,
                explored_nodes: total_nodes,
            })
            .collect(),
    )
}

fn initial_branches(grid: &Grid, candidates: &[CandidateAnswer], branch_limit: usize, seed: u64) -> Vec<Grid> {
    let mut roots = Vec::new();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut anchors: Vec<_> = candidates.iter().take(branch_limit * 3).collect();
    anchors.shuffle(&mut rng);
    anchors.sort_by_key(|candidate| std::cmp::Reverse(candidate.quality_score));

    for candidate in anchors.into_iter().take(branch_limit) {
        let start_x = grid.width.saturating_sub(candidate.normalized.len()) / 2;
        let row = grid.height / 2;
        if let Some(placed) = grid.place(candidate, start_x, row, Direction::Across) {
            roots.push(placed);
        }
        if roots.len() >= branch_limit {
            break;
        }
    }

    roots
}

fn dfs(
    grid: Grid,
    context: &SearchContext<'_>,
    depth: usize,
    explored: &mut usize,
    rng: &mut ChaCha8Rng,
) -> Vec<Grid> {
    if Instant::now() >= context.deadline {
        return vec![grid];
    }

    let mut best = vec![grid.clone()];
    let mut seen = HashSet::from([grid_signature(&grid)]);
    let mut next_moves = enumerate_moves(&grid, context.candidates, context.branch_limit);
    next_moves.shuffle(rng);
    next_moves.sort_by(|left, right| right.4.cmp(&left.4));

    for (candidate, x, y, direction, _) in next_moves.into_iter().take(branching_cap(depth, context.branch_limit)) {
        if Instant::now() >= context.deadline {
            break;
        }
        if let Some(next_grid) = grid.place(candidate, x, y, direction) {
            *explored += 1;
            for result in dfs(next_grid, context, depth + 1, explored, rng) {
                insert_top_grid(&mut best, &mut seen, result, context.top_n);
            }
        }
    }

    best
}

fn enumerate_moves<'a>(
    grid: &Grid,
    candidates: &'a [CandidateAnswer],
    branch_limit: usize,
) -> Vec<(&'a CandidateAnswer, usize, usize, Direction, i32)> {
    let mut moves = Vec::new();
    for candidate in candidates.iter().take(branch_limit * 12) {
        for (x, y, direction, crossings) in grid.placements_for(candidate) {
            let weight = candidate.quality_score
                + (crossings as i32 * 25)
                + multi_crossing_move_bonus(crossings)
                + (candidate.normalized.len() as i32 * 5)
                - metadata_blank_repeat_penalty(candidate, grid)
                - snippet_move_penalty(candidate, grid);
            moves.push((candidate, x, y, direction, weight));
        }
    }
    moves
}

fn branching_cap(depth: usize, branch_limit: usize) -> usize {
    match depth {
        0 | 1 => branch_limit,
        2 => branch_limit / 2 + 1,
        _ => 6,
    }
}

fn insert_top_grid(best: &mut Vec<Grid>, seen: &mut HashSet<String>, grid: Grid, limit: usize) {
    let signature = grid_signature(&grid);
    if !seen.insert(signature.clone()) {
        return;
    }

    best.push(grid);
    best.sort_by_key(|grid| std::cmp::Reverse(score_grid(grid)));
    if best.len() > limit {
        if let Some(removed) = best.pop() {
            seen.remove(&signature_for_removal(&removed));
        }
    }
}

fn grid_signature(grid: &Grid) -> String {
    let mut entries = grid
        .entries
        .iter()
        .map(|entry| {
            format!(
                "{}:{}:{}:{}:{}",
                entry.x,
                entry.y,
                match entry.direction {
                    Direction::Across => "A",
                    Direction::Down => "D",
                },
                entry.candidate.track_index,
                entry.candidate.normalized
            )
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries.join("|")
}

fn signature_for_removal(grid: &Grid) -> String {
    grid_signature(grid)
}

fn score_grid(grid: &Grid) -> Score {
    let snippet_entries = grid
        .entries
        .iter()
        .filter(|entry| matches!(entry.candidate.source_kind, SourceKind::Snippet))
        .count();
    let metadata_blank_entries = grid
        .entries
        .iter()
        .filter(|entry| entry.candidate.is_metadata_fill_in_blank())
        .count();
    let hub_entries = grid
        .entries
        .iter()
        .filter(|entry| entry.crossings > 1)
        .count();
    Score {
        filled_cells: grid.filled_count(),
        crossings: grid.entries.iter().map(|entry| entry.crossings).sum(),
        hub_entries,
        title_entries: grid
            .entries
            .iter()
            .filter(|entry| matches!(entry.candidate.source_kind, SourceKind::Title))
            .count(),
        snippet_entries,
        metadata_blank_entries,
        clue_quality: grid.entries.iter().map(|entry| entry.candidate.quality_score).sum::<i32>()
            + hub_entry_bonus(hub_entries, grid.entries.iter().map(|entry| entry.crossings).sum())
            - metadata_blank_layout_penalty(grid)
            - snippet_layout_penalty(snippet_entries),
    }
}

fn multi_crossing_move_bonus(crossings: usize) -> i32 {
    if crossings <= 1 {
        return 0;
    }
    let extra_crossings = (crossings - 1) as i32;
    40 + extra_crossings * 35
}

fn snippet_move_penalty(candidate: &CandidateAnswer, grid: &Grid) -> i32 {
    if !matches!(candidate.source_kind, SourceKind::Snippet) {
        return 0;
    }

    let existing_snippets = grid
        .entries
        .iter()
        .filter(|entry| matches!(entry.candidate.source_kind, SourceKind::Snippet))
        .count() as i32;
    35 + existing_snippets * 18
}

fn metadata_blank_repeat_penalty(candidate: &CandidateAnswer, grid: &Grid) -> i32 {
    if !candidate.is_metadata_fill_in_blank() {
        return 0;
    }

    let existing_metadata_blanks = grid
        .entries
        .iter()
        .filter(|entry| entry.candidate.is_metadata_fill_in_blank())
        .count() as i32;
    let same_kind_blanks = grid
        .entries
        .iter()
        .filter(|entry| {
            entry.candidate.is_metadata_fill_in_blank()
                && entry.candidate.source_kind == candidate.source_kind
        })
        .count() as i32;

    20 + existing_metadata_blanks * 16 + same_kind_blanks * 24
}

fn snippet_layout_penalty(snippet_entries: usize) -> i32 {
    let snippets = snippet_entries as i32;
    if snippets <= 1 {
        return snippets * 10;
    }
    10 + (snippets - 1) * 45
}

fn hub_entry_bonus(hub_entries: usize, total_crossings: usize) -> i32 {
    let hubs = hub_entries as i32;
    let extra_crossings = total_crossings.saturating_sub(hub_entries) as i32;
    hubs * 60 + extra_crossings * 20
}

fn metadata_blank_layout_penalty(grid: &Grid) -> i32 {
    let metadata_blanks = grid
        .entries
        .iter()
        .filter(|entry| entry.candidate.is_metadata_fill_in_blank())
        .collect::<Vec<_>>();
    if metadata_blanks.len() <= 1 {
        return 0;
    }

    let repeated_total = (metadata_blanks.len() - 1) as i32 * 28;
    let repeated_same_kind = [SourceKind::Title, SourceKind::Artist, SourceKind::Album]
        .into_iter()
        .map(|kind| {
            let count = metadata_blanks
                .iter()
                .filter(|entry| entry.candidate.source_kind == kind)
                .count();
            count.saturating_sub(1) as i32
        })
        .sum::<i32>();
    repeated_total + repeated_same_kind * 24
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        derive::{SourceKind, derive_candidates},
        grid::{Direction, Grid},
        input::PuzzleInput,
    };

    use super::{
        SearchConfig, hub_entry_bonus, metadata_blank_layout_penalty, metadata_blank_repeat_penalty,
        multi_crossing_move_bonus, score_grid, search_best_n,
    };

    #[test]
    fn finds_a_solution_for_small_input() {
        let input = PuzzleInput {
            width: 7,
            height: 7,
            tracks: vec![
                crate::input::Track {
                    name: "Roxanne".to_string(),
                    artist: "Police".to_string(),
                    album: Some("Outlandos".to_string()),
                    snippets: vec!["Red light".to_string()],
                    custom: Vec::new(),
                },
                crate::input::Track {
                    name: "Message".to_string(),
                    artist: "Police".to_string(),
                    album: Some("Regatta".to_string()),
                    snippets: vec!["Bottle".to_string()],
                    custom: Vec::new(),
                },
            ],
        };
        let candidates = derive_candidates(&input).unwrap();
        let config = SearchConfig {
            time_limit: Duration::from_millis(200),
            seed: 7,
            branch_limit: 12,
            top_n: 1,
        };
        let solutions = search_best_n(&input, &candidates, &config);
        assert!(solutions.is_some());
    }

    #[test]
    fn returns_ranked_top_n_solutions() {
        let input = PuzzleInput {
            width: 8,
            height: 8,
            tracks: vec![
                crate::input::Track {
                    name: "Roxanne".to_string(),
                    artist: "The Police".to_string(),
                    album: Some("Outlandos".to_string()),
                    snippets: vec!["Red light".to_string()],
                    custom: Vec::new(),
                },
                crate::input::Track {
                    name: "Regatta".to_string(),
                    artist: "Sting".to_string(),
                    album: Some("Police".to_string()),
                    snippets: vec!["Bottle message".to_string()],
                    custom: Vec::new(),
                },
                crate::input::Track {
                    name: "Synchronicity".to_string(),
                    artist: "Police".to_string(),
                    album: Some("Ghost".to_string()),
                    snippets: vec!["Every breath".to_string()],
                    custom: Vec::new(),
                },
            ],
        };
        let candidates = derive_candidates(&input).unwrap();
        let config = SearchConfig {
            time_limit: Duration::from_millis(250),
            seed: 3,
            branch_limit: 12,
            top_n: 3,
        };
        let solutions = search_best_n(&input, &candidates, &config).unwrap();
        assert!(!solutions.is_empty());
        assert!(solutions.len() <= 3);
        for pair in solutions.windows(2) {
            assert!(pair[0].score >= pair[1].score);
        }
    }

    #[test]
    fn grid_scoring_penalizes_lyric_heavy_layouts() {
        let input = PuzzleInput {
            width: 12,
            height: 12,
            tracks: vec![
                crate::input::Track {
                    name: "The Modern Age".to_string(),
                    artist: "The Strokes".to_string(),
                    album: Some("Is This It".to_string()),
                    snippets: vec!["No time to feel the breeze".to_string()],
                    custom: Vec::new(),
                },
                crate::input::Track {
                    name: "Last Nite".to_string(),
                    artist: "The Strokes".to_string(),
                    album: Some("Is This It".to_string()),
                    snippets: vec!["I know this for sure".to_string()],
                    custom: Vec::new(),
                },
            ],
        };
        let candidates = derive_candidates(&input).unwrap();
        let title = candidates
            .iter()
            .find(|candidate| candidate.track_index == 0 && matches!(candidate.source_kind, SourceKind::Title))
            .unwrap();
        let snippet = candidates
            .iter()
            .find(|candidate| candidate.track_index == 1 && matches!(candidate.source_kind, SourceKind::Snippet))
            .unwrap();

        let title_grid = Grid::new(input.width, input.height)
            .place(title, 0, 0, Direction::Across)
            .unwrap();
        let snippet_grid = Grid::new(input.width, input.height)
            .place(snippet, 0, 0, Direction::Across)
            .unwrap();

        assert!(score_grid(&title_grid) > score_grid(&snippet_grid));
    }

    #[test]
    fn multi_crossing_entries_get_density_bonus() {
        assert_eq!(multi_crossing_move_bonus(0), 0);
        assert_eq!(multi_crossing_move_bonus(1), 0);
        assert!(multi_crossing_move_bonus(2) > 0);
        assert!(multi_crossing_move_bonus(3) > multi_crossing_move_bonus(2));
        assert!(hub_entry_bonus(2, 5) > hub_entry_bonus(1, 2));
    }

    #[test]
    fn repeated_metadata_fill_in_blank_gets_penalized() {
        let input = PuzzleInput {
            width: 18,
            height: 18,
            tracks: vec![
                crate::input::Track {
                    name: "The Modern Age".to_string(),
                    artist: "The Strokes".to_string(),
                    album: Some("The New Abnormal".to_string()),
                    snippets: vec![],
                    custom: Vec::new(),
                },
                crate::input::Track {
                    name: "The Adults Are Talking".to_string(),
                    artist: "The Strokes".to_string(),
                    album: Some("Is This It".to_string()),
                    snippets: vec![],
                    custom: Vec::new(),
                },
            ],
        };
        let candidates = derive_candidates(&input).unwrap();
        let first_title_blank = candidates
            .iter()
            .find(|candidate| candidate.normalized == "MODERN")
            .unwrap();
        let second_title_blank = candidates
            .iter()
            .find(|candidate| candidate.normalized == "ADULTS")
            .unwrap();

        let grid = Grid::new(input.width, input.height)
            .place(first_title_blank, 0, 2, Direction::Across)
            .unwrap();

        assert!(first_title_blank.is_metadata_fill_in_blank());
        assert!(second_title_blank.is_metadata_fill_in_blank());
        assert!(metadata_blank_repeat_penalty(second_title_blank, &grid) > 0);
        assert!(metadata_blank_layout_penalty(&grid) == 0);

        let (x, y, direction, _) = grid.placements_for(second_title_blank).into_iter().next().unwrap();
        let expanded = grid
            .place(second_title_blank, x, y, direction)
            .unwrap();
        assert!(metadata_blank_layout_penalty(&expanded) > 0);
        assert_eq!(score_grid(&expanded).metadata_blank_entries, 2);
    }
}
