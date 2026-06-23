use std::sync::Arc;

use console::style;
use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use gogdl_lib::{GogDl, games::GameBuild};

/// Lists builds via `download::list_builds` and lets the user pick one.
/// Highlights index 0 as "(latest)" and, if `current_version` matches a
/// build's `version_name`, tags it "(installed)". Returns `None` if the
/// fetch fails, the list is empty, or the user backs out.
pub async fn select_build_interactive(
    gogdl: Arc<GogDl>,
    game_id: i32,
    current_version: Option<&str>,
) -> Option<GameBuild> {
    let builds = match crate::commands::download::list_builds(gogdl, game_id).await {
        Ok(builds) => builds,
        Err(err) => {
            println!("{}", style(format!("Error fetching builds: {}", err)).red());
            return None;
        }
    };

    if builds.is_empty() {
        println!("{}", style("No builds found for this game.").yellow());
        return None;
    }

    let mut options: Vec<String> = builds
        .iter()
        .enumerate()
        .map(|(idx, build)| {
            let mut tags = Vec::new();
            if idx == 0 {
                tags.push("latest");
            }
            if current_version == Some(build.version_name.as_str()) {
                tags.push("installed");
            }
            let suffix = if tags.is_empty() {
                String::new()
            } else {
                format!("  ({})", tags.join(", "))
            };
            format!(
                "{}  ({}){}",
                build.version_name, build.date_published, suffix
            )
        })
        .collect();
    options.push("<- Cancel".to_string());

    println!();
    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a build")
        .items(&options)
        .default(0)
        .interact_opt();

    match selection {
        Ok(Some(idx)) if idx < builds.len() => builds.into_iter().nth(idx),
        _ => None,
    }
}

pub async fn find_executables(base_path: &str) -> Vec<String> {
    let mut executables = Vec::new();

    if let Ok(_entries) = tokio::fs::read_dir(base_path).await {
        let mut stack = vec![String::new()];

        while let Some(relative_dir) = stack.pop() {
            let current_path = if relative_dir.is_empty() {
                base_path.to_string()
            } else {
                format!("{}/{}", base_path, relative_dir)
            };

            if let Ok(mut dir_entries) = tokio::fs::read_dir(&current_path).await {
                while let Ok(Some(entry)) = dir_entries.next_entry().await {
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    let relative_path = if relative_dir.is_empty() {
                        file_name.clone()
                    } else {
                        format!("{}/{}", relative_dir, file_name)
                    };

                    if let Ok(file_type) = entry.file_type().await {
                        if file_type.is_dir() {
                            // Skip common non-game directories
                            let skip_dirs = [
                                "support",
                                "__support",
                                "directx",
                                "redist",
                                "vcredist",
                                "_commonredist",
                            ];
                            if !skip_dirs
                                .iter()
                                .any(|d| file_name.to_lowercase().contains(d))
                            {
                                stack.push(relative_path);
                            }
                        } else if file_type.is_file() {
                            let lower = file_name.to_lowercase();
                            if lower.ends_with(".exe") {
                                // Skip common non-game executables
                                let skip_exes = [
                                    "unins",
                                    "setup",
                                    "install",
                                    "crash",
                                    "report",
                                    "vc_redist",
                                    "dxsetup",
                                    "dotnet",
                                ];
                                if !skip_exes.iter().any(|s| lower.contains(s)) {
                                    executables.push(relative_path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    executables.sort();
    executables.dedup();
    executables
}
