use std::fs;

use crate::commands::path::{candidate_log_paths, display_path};

pub async fn logs(limit: usize) -> anyhow::Result<()> {
    let limit = limit.clamp(10, 500);
    let mut found_any = false;

    println!("\n========== Titan Logs ==========\n");

    for path in candidate_log_paths() {
        if !path.exists() {
            continue;
        }

        found_any = true;
        let content = fs::read_to_string(&path)?;
        let mut lines: Vec<&str> = content.lines().collect();
        if lines.len() > limit {
            lines = lines.split_off(lines.len() - limit);
        }

        println!("Path: {}", display_path(&path));
        println!("Showing last {} line(s)\n", lines.len());
        for line in lines {
            println!("{}", line);
        }
        println!();
    }

    if !found_any {
        println!("No log files found under {}", display_path(crate::commands::path::data_dir()));
    }

    Ok(())
}
