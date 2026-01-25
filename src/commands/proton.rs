use std::io::Write;
use std::sync::Arc;

use console::style;
use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use prefix_manager::PrefixManager;
use prefix_manager::api::releases::Release;

use crate::settings::AppSettings;

pub async fn handle_proton(settings: &mut AppSettings) {
    loop {
        println!();
        println!("{}", style("🍷 Proton Version Manager").bold().cyan());
        println!("{}", style("Manage your Proton/Wine versions").dim());
        println!();

        let options = vec![
            "Browse & download new versions",
            "View installed versions",
            "Remove an installed version",
            "← Back / Exit",
        ];

        let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("What would you like to do?")
            .items(&options)
            .default(0)
            .interact_opt();

        match selection {
            Ok(Some(0)) => browse_versions(settings).await,
            Ok(Some(1)) => view_installed_versions(settings).await,
            Ok(Some(2)) => remove_version(settings).await,
            Ok(Some(3)) | Ok(None) | Err(_) => {
                println!("{}", style("Goodbye!").green());
                break;
            }
            _ => {}
        }
    }
}

async fn browse_versions(settings: &mut AppSettings) {
    let prefix_manager = PrefixManager::new_with_default_client();
    let mut current_page = 1;

    loop {
        println!();
        println!(
            "{}",
            style(format!(
                "📦 Available Proton Versions (Page {})",
                current_page
            ))
            .bold()
            .cyan()
        );
        println!("{}", style("Select a version to download").dim());
        println!();

        let releases = match prefix_manager.get_releases(current_page).await {
            Ok(releases) => releases,
            Err(err) => {
                println!(
                    "{}",
                    style(format!("Failed to fetch releases: {}", err)).red()
                );
                return;
            }
        };

        if releases.is_empty() {
            println!("{}", style("No more versions available.").yellow());
            if current_page > 1 {
                current_page -= 1;
                continue;
            }
            return;
        }

        // Build the list of versions with installed status
        let version_options: Vec<String> = releases
            .iter()
            .map(|r| {
                let is_installed = settings
                    .downloaded_proton_versions
                    .iter()
                    .any(|v| v.version == r.tag_name);

                let size_str = r
                    .get_download_size()
                    .map(|s| format!(" ({} MB)", s / 1024 / 1024))
                    .unwrap_or_default();

                if is_installed {
                    format!(
                        "{}{} {}",
                        r.tag_name,
                        size_str,
                        style("[installed]").green()
                    )
                } else {
                    format!("{}{}", r.tag_name, size_str)
                }
            })
            .collect();

        let mut options = version_options.clone();
        if current_page > 1 {
            options.push(style("← Previous page").dim().to_string());
        }
        options.push(style("→ Next page").dim().to_string());
        options.push(style("← Back").dim().to_string());

        let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a version")
            .items(&options)
            .default(0)
            .interact_opt();

        match selection {
            Ok(Some(idx)) if idx < releases.len() => {
                let selected_release = &releases[idx];

                // Check if already installed
                let is_installed = settings
                    .downloaded_proton_versions
                    .iter()
                    .any(|v| v.version == selected_release.tag_name);

                if is_installed {
                    println!();
                    println!(
                        "{}",
                        style(format!(
                            "{} is already installed!",
                            selected_release.tag_name
                        ))
                        .yellow()
                    );
                    continue;
                }

                // Confirm download
                println!();
                println!(
                    "{}",
                    style(format!("📥 Downloading {}...", selected_release.tag_name)).cyan()
                );

                download_version(settings, selected_release.clone()).await;
            }
            Ok(Some(idx)) => {
                let option_idx = idx - releases.len();
                let has_prev = current_page > 1;

                if has_prev && option_idx == 0 {
                    current_page -= 1;
                } else if (has_prev && option_idx == 1) || (!has_prev && option_idx == 0) {
                    current_page += 1;
                } else {
                    break;
                }
            }
            Ok(None) | Err(_) => break,
        }
    }
}

async fn download_version(settings: &mut AppSettings, release: Release) {
    let prefix_manager = PrefixManager::new_with_default_client();
    let prefix_manager_arc = Arc::new(prefix_manager);

    let checksum = release.get_checksum();
    let total_size = release.get_download_size().unwrap_or(0);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<i64>();

    let proton_path = format!("{}/proton", settings.data_path);
    let proton_path_clone = proton_path.clone();
    let release_clone = release.clone();

    // Spawn download task
    tokio::spawn(async move {
        let _ = prefix_manager_arc
            .download_release(&release_clone, &proton_path_clone, checksum, tx)
            .await;
    });

    // Progress reporting
    let mut downloaded_size: i64 = 0;
    while let Some(size) = rx.recv().await {
        downloaded_size += size;
        print!(
            "\rDownloaded: {} MB/{} MB -- {:.2}%",
            downloaded_size / 1024 / 1024,
            total_size / 1024 / 1024,
            downloaded_size as f64 / total_size as f64 * 100.0
        );
        let _ = std::io::stdout().flush();
    }

    println!(); // New line after progress

    // Save to settings
    settings
        .add_proton_version(
            &release.tag_name,
            &format!("{}/{}", &proton_path, release.tag_name),
        )
        .await;

    println!();
    println!(
        "{}",
        style(format!("✅ {} installed successfully!", release.tag_name)).green()
    );
}

async fn view_installed_versions(settings: &AppSettings) {
    println!();
    println!("{}", style("📋 Installed Proton Versions").bold().cyan());
    println!();

    if settings.downloaded_proton_versions.is_empty() {
        println!(
            "{}",
            style("No Proton versions installed yet. Use 'Browse & download' to get started!")
                .yellow()
        );
        return;
    }

    for (i, version) in settings.downloaded_proton_versions.iter().enumerate() {
        println!(
            "  {}. {} {}",
            style(i + 1).dim(),
            style(&version.version).green(),
            style(format!("({})", version.path)).dim()
        );
    }

    println!();
    println!(
        "{}",
        style(format!(
            "Total: {} version(s) installed",
            settings.downloaded_proton_versions.len()
        ))
        .dim()
    );
}

async fn remove_version(settings: &mut AppSettings) {
    if settings.downloaded_proton_versions.is_empty() {
        println!();
        println!(
            "{}",
            style("No Proton versions installed to remove.").yellow()
        );
        return;
    }

    println!();
    println!("{}", style("🗑️  Remove Proton Version").bold().cyan());
    println!(
        "{}",
        style("Select a version to remove (files will be deleted)").dim()
    );
    println!();

    let version_options: Vec<String> = settings
        .downloaded_proton_versions
        .iter()
        .map(|v| {
            // Check if any game is using this version
            let in_use = settings.downloaded_games.iter().any(|g| {
                g.proton_version
                    .as_ref()
                    .map(|pv| pv.version == v.version)
                    .unwrap_or(false)
            });

            if in_use {
                format!("{} {}", v.version, style("[in use]").yellow())
            } else {
                v.version.clone()
            }
        })
        .collect();

    let mut options = version_options.clone();
    options.push(style("← Cancel").dim().to_string());

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select version to remove")
        .items(&options)
        .default(0)
        .interact_opt();

    match selection {
        Ok(Some(idx)) if idx < settings.downloaded_proton_versions.len() => {
            let version = &settings.downloaded_proton_versions[idx];
            let version_name = version.version.clone();
            let version_path = version.path.clone();

            // Check if in use
            let in_use = settings.downloaded_games.iter().any(|g| {
                g.proton_version
                    .as_ref()
                    .map(|pv| pv.version == version_name)
                    .unwrap_or(false)
            });

            if in_use {
                println!();
                println!(
                    "{}",
                    style(format!(
                        "⚠️  {} is currently in use by one or more games.",
                        version_name
                    ))
                    .yellow()
                );

                let confirm_options = vec!["Yes, remove anyway", "No, cancel"];
                let confirm = FuzzySelect::with_theme(&ColorfulTheme::default())
                    .with_prompt("Are you sure you want to remove it?")
                    .items(&confirm_options)
                    .default(1)
                    .interact_opt();

                match confirm {
                    Ok(Some(0)) => {}
                    _ => {
                        println!("{}", style("Cancelled").dim());
                        return;
                    }
                }

                // Clear proton version from affected games
                for game in settings.downloaded_games.iter_mut() {
                    if game
                        .proton_version
                        .as_ref()
                        .map(|pv| pv.version == version_name)
                        .unwrap_or(false)
                    {
                        game.proton_version = None;
                    }
                }
            }

            // Delete the directory
            println!();
            println!("{}", style(format!("Removing {}...", version_name)).dim());

            match tokio::fs::remove_dir_all(&version_path).await {
                Ok(_) => {
                    // Remove from settings
                    settings
                        .downloaded_proton_versions
                        .retain(|v| v.version != version_name);
                    let _ = settings.save().await;

                    println!(
                        "{}",
                        style(format!("✅ {} removed successfully!", version_name)).green()
                    );
                }
                Err(err) => {
                    println!(
                        "{}",
                        style(format!("❌ Failed to remove files: {}", err)).red()
                    );
                    println!(
                        "{}",
                        style("The version will be removed from the list, but files may remain.")
                            .yellow()
                    );

                    // Still remove from settings
                    settings
                        .downloaded_proton_versions
                        .retain(|v| v.version != version_name);
                    let _ = settings.save().await;
                }
            }
        }
        _ => {
            println!("{}", style("Cancelled").dim());
        }
    }
}
