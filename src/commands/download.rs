use std::{io::Write, process::exit, sync::Arc, time::Instant};

use chrono::{DateTime, Utc};
use gogdl_lib::{
    GogDl, GogdlError,
    client::ClientError,
    games::{DownloadOptions, OperatingSystem, RepairProgress, RepairSummary},
};
use tokio_util::sync::CancellationToken;

use crate::{auth::manage_auth, settings::AppSettings};

pub async fn handle_download(
    gogdl: Arc<GogDl>,
    game_id: i32,
    version_id: Option<String>,
    path: &str,
    settings: &mut AppSettings,
    fix: bool,
) -> Result<(), anyhow::Error> {
    let mut download_build = version_id.clone().unwrap_or_default();
    let game_details = gogdl.get_game_details(game_id).await?;

    let result = {
        if let Some(version_id) = version_id {
            if settings.downloaded_games.iter().any(|game| {
                game.game_id == game_id
                    && game.build_id == version_id
                    && game.download_complete
                    && !fix
            }) {
                println!("Game already downloaded");
                return Ok(());
            }
            download_game(gogdl.clone(), game_id, &version_id, path).await
        } else {
            let game_builds = match gogdl
                .get_game_builds(game_id, OperatingSystem::Windows)
                .await
            {
                Ok(builds) => builds.items,
                Err(err) => {
                    println!("Error fetching game builds: {}", err);
                    exit(1)
                }
            };

            let latest_build = game_builds
                .iter()
                .filter_map(|b| {
                    DateTime::parse_from_str(&b.date_published, "%Y-%m-%dT%H:%M:%S%z")
                        .ok()
                        .map(|dt| (dt.with_timezone(&Utc), b))
                })
                .max_by_key(|(dt, _)| *dt)
                .map(|(_, b)| b);

            if let Some(latest) = latest_build {
                download_build = latest.version_name.clone();
                if settings.downloaded_games.iter().any(|game| {
                    game.game_id == game_id
                        && game.build_id == download_build
                        && game.download_complete
                        && !fix
                }) {
                    println!("Game already downloaded");
                    return Ok(());
                }
                download_game(gogdl.clone(), game_id, &download_build, path).await
            } else {
                println!("Could not fetch latest build");
                exit(1)
            }
        }
    };

    let complete = match result {
        Ok(complete) => complete,
        Err(err) => {
            if let GogdlError::ClientError(ClientError::Http { status, .. }) = &err {
                if status.as_u16() == 401 {
                    manage_auth(gogdl.clone()).await;
                }
            }
            return Err(err.into());
        }
    };

    settings
        .add_game(
            &download_build,
            &format!("{}/{}", path, game_details.title),
            None,
            complete,
            game_id,
        )
        .await;
    Ok(())
}

pub async fn download_game(
    gogdl: Arc<GogDl>,
    game_id: i32,
    build_name: &str,
    path: &str,
) -> Result<bool, GogdlError> {
    let estimate = gogdl
        .estimate_download(game_id, OperatingSystem::Windows, build_name, path)
        .await?;
    let total_size = estimate.total_size as u64;
    let already_present = estimate.already_present.max(0) as u64;

    println!("Total size: {} MB", total_size / 1024 / 1024);
    if estimate.already_present > 0 {
        let pct = (already_present as f64 / total_size.max(1) as f64) * 100.0;
        println!(
            "Already on disk: {} MB ({:.1}%) — resuming",
            already_present / 1024 / 1024,
            pct
        );
    }
    println!(
        "Remaining to download: {} MB",
        estimate.remaining.max(0) / 1024 / 1024
    );

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(i64, i64)>();
    let build_name_clone = build_name.to_string();
    let path_clone = path.to_string();

    // Spawn the download task
    let download_task = tokio::spawn(async move {
        gogdl
            .download_build(
                game_id,
                &build_name_clone,
                tx,
                &path_clone,
                OperatingSystem::Windows,
                CancellationToken::new(),
                DownloadOptions::default(),
            )
            .await
    });

    // Progress reporting loop
    let mut downloaded_size: i64 = 0;
    let start = Instant::now();
    let already_present_baseline = already_present as i64;

    while let Some((downloaded, _total)) = rx.recv().await {
        downloaded_size = downloaded;
        let percent = ((downloaded_size as f64 / total_size as f64) * 100.0) as f64;

        // Only update display when percentage changes
        let downloaded_mb = downloaded_size / 1024 / 1024;
        let total_mb = total_size / 1024 / 1024;

        let elapsed_secs = start.elapsed().as_secs_f64();
        let session_bytes = (downloaded_size - already_present_baseline).max(0) as f64;
        let speed_suffix = if elapsed_secs > 0.5 && session_bytes > 0.0 {
            let speed = session_bytes / elapsed_secs;
            let remaining_bytes = (total_size as i64 - downloaded_size).max(0) as f64;
            let eta = if speed > 0.0 {
                format_duration(remaining_bytes / speed)
            } else {
                "--".to_string()
            };
            format!("  {}/s, ETA {}", format_speed(speed), eta)
        } else {
            String::new()
        };

        print!(
            "\r{} [{:50}] {:.2}%  ({:.2} MB / {:.2} MB){}",
            console::style("Progress:").green(),
            "=".repeat((percent as usize) / 2),
            percent,
            downloaded_mb,
            total_mb,
            speed_suffix
        );

        // Flush stdout to ensure progress is displayed immediately
        let _ = std::io::stdout().flush();
    }

    println!(); // New line after progress

    match download_task.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            println!("Error downloading build: {}", err);
            return Err(err);
        }
        Err(join_err) => {
            println!("Download task failed: {}", join_err);
            return Ok(false);
        }
    }

    if downloaded_size >= total_size as i64 {
        println!("Download complete!");
        Ok(true)
    } else {
        println!(
            "Download incomplete! ({}/{} bytes)",
            downloaded_size, total_size
        );
        Ok(false)
    }
}

/// Single-pass verify/repair: drives `GogDl::verify_and_repair_build` and
/// renders one continuous progress bar by combining `Verifying::verified`
/// with `Downloading::downloaded`, per that API's own progress contract
/// (`Downloading::downloaded` is session-only and its `total` is the whole
/// build, not "remaining to repair" — adding it to the latest verified count
/// is what keeps the percentage meaningful).
pub async fn verify_and_repair_game(
    gogdl: Arc<GogDl>,
    game_id: i32,
    build_name: &str,
    path: &str,
) -> Result<RepairSummary, GogdlError> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<RepairProgress>();
    let build_name_clone = build_name.to_string();
    let path_clone = path.to_string();

    let task = tokio::spawn(async move {
        gogdl
            .verify_and_repair_build(
                game_id,
                &build_name_clone,
                tx,
                &path_clone,
                OperatingSystem::Windows,
                CancellationToken::new(),
                DownloadOptions::default(),
            )
            .await
    });

    let mut total_size: i64 = 0;
    let mut last_verified: i64 = 0;
    let mut printed_total = false;
    let start = Instant::now();

    while let Some(event) = rx.recv().await {
        match event {
            RepairProgress::Verifying { verified, total } => {
                total_size = total;
                last_verified = verified;

                if !printed_total {
                    println!("Total size: {} MB", total_size / 1024 / 1024);
                    printed_total = true;
                }

                let percent = (last_verified as f64 / total_size.max(1) as f64) * 100.0;
                print!(
                    "\r{} [{:50}] {:.2}%  ({:.2} MB / {:.2} MB)",
                    console::style("Verifying:").green(),
                    "=".repeat((percent as usize) / 2),
                    percent,
                    last_verified as f64 / 1024.0 / 1024.0,
                    total_size as f64 / 1024.0 / 1024.0,
                );
                let _ = std::io::stdout().flush();
            }
            RepairProgress::Downloading { downloaded, .. } => {
                let combined = last_verified + downloaded;
                let percent = (combined as f64 / total_size.max(1) as f64) * 100.0;

                let elapsed_secs = start.elapsed().as_secs_f64();
                let speed_suffix = if elapsed_secs > 0.5 && downloaded > 0 {
                    format!("  {}/s", format_speed(downloaded as f64 / elapsed_secs))
                } else {
                    String::new()
                };

                print!(
                    "\r{} [{:50}] {:.2}%  ({:.2} MB / {:.2} MB){}",
                    console::style("Repairing:").yellow(),
                    "=".repeat((percent as usize) / 2),
                    percent,
                    combined as f64 / 1024.0 / 1024.0,
                    total_size as f64 / 1024.0 / 1024.0,
                    speed_suffix
                );
                let _ = std::io::stdout().flush();
            }
        }
    }

    println!(); // New line after progress

    let summary = match task.await {
        Ok(inner) => inner?,
        Err(join_err) => return Err(ClientError::AsyncError(join_err).into()),
    };

    print_repair_summary(&summary);

    Ok(summary)
}

fn print_repair_summary(summary: &RepairSummary) {
    println!("Verification complete!");
    println!("Total size: {} MB", summary.total_size / 1024 / 1024);
    let pct = (summary.already_valid as f64 / summary.total_size.max(1) as f64) * 100.0;
    println!(
        "Already valid: {} MB ({:.1}%)",
        summary.already_valid / 1024 / 1024,
        pct
    );
    println!("Repaired: {} MB", summary.repaired / 1024 / 1024);

    if summary.repaired_files.is_empty() {
        println!("No files needed repair.");
    } else {
        println!("Repaired files ({}):", summary.repaired_files.len());
        for file in summary.repaired_files.iter().take(10) {
            println!("  - {}", file);
        }
        if summary.repaired_files.len() > 10 {
            println!("  + {} more", summary.repaired_files.len() - 10);
        }
    }
}

fn format_speed(bytes_per_sec: f64) -> String {
    if bytes_per_sec < 1024.0 * 1024.0 {
        format!("{:.0} KB", bytes_per_sec / 1024.0)
    } else {
        format!("{:.1} MB", bytes_per_sec / (1024.0 * 1024.0))
    }
}

fn format_duration(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "--".to_string();
    }
    let total_secs = secs.round() as u64;
    let m = total_secs / 60;
    let s = total_secs % 60;
    if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}
