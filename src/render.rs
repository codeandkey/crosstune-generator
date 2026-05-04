use crate::{
    grid::{Direction, NumberedEntry},
    search::{Score, SearchConfig, Solution},
};

pub fn render_puzzles(solutions: &[Solution], config: &SearchConfig) -> String {
    let mut out = String::new();
    for (index, solution) in solutions.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&format!("Solution {}\n", index + 1));
        out.push_str("Grid\n");
        for y in 0..solution.grid.height {
            for x in 0..solution.grid.width {
                out.push(solution.grid.cell(x, y).unwrap_or('■'));
                if x + 1 < solution.grid.width {
                    out.push(' ');
                }
            }
            out.push('\n');
        }

        let entries = solution.grid.numbered_entries();
        let (across, down): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .partition(|entry| matches!(entry.direction, Direction::Across));

        out.push('\n');
        render_section(&mut out, "Across", &across);
        out.push('\n');
        render_section(&mut out, "Down", &down);
        out.push('\n');
        render_stats(&mut out, solution.score, solution.explored_nodes, solution.elapsed, config);
    }
    out
}

fn render_section(out: &mut String, title: &str, entries: &[NumberedEntry]) {
    out.push_str(title);
    out.push('\n');
    for entry in entries {
        out.push_str(&format!(
            "{}. ({},{}) [{}] {} | answer={} | source={}\n",
            entry.number,
            entry.x + 1,
            entry.y + 1,
            match entry.direction {
                Direction::Across => "A",
                Direction::Down => "D",
            },
            entry.clue,
            entry.answer,
            entry.source
        ));
    }
}

fn render_stats(
    out: &mut String,
    score: Score,
    explored_nodes: usize,
    elapsed: std::time::Duration,
    config: &SearchConfig,
) {
    out.push_str("Stats\n");
    out.push_str(&format!(
        "filled_cells={} crossings={} hub_entries={} title_entries={} snippet_entries={} metadata_blank_entries={} clue_quality={} explored_nodes={} elapsed_ms={} seed={} branch_limit={} time_limit_ms={}\n",
        score.filled_cells,
        score.crossings,
        score.hub_entries,
        score.title_entries,
        score.snippet_entries,
        score.metadata_blank_entries,
        score.clue_quality,
        explored_nodes,
        elapsed.as_millis(),
        config.seed,
        config.branch_limit,
        config.time_limit.as_millis()
    ));
}
