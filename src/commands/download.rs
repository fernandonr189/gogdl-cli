use std::{
    cmp::Reverse,
    collections::VecDeque,
    io::Write,
    process::exit,
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use gogdl_lib::{
    GogDl, GogdlError,
    client::ClientError,
    games::{
        CleanupSummary, DownloadOptions, GameBuild, OperatingSystem, RepairProgress, RepairSummary,
    },
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
            let latest_build = match find_latest_build(gogdl.clone(), game_id).await {
                Ok(build) => build,
                Err(err) => {
                    println!("Error fetching game builds: {}", err);
                    exit(1)
                }
            };

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

/// Fetches all builds for a game, sorted by `date_published` descending
/// (latest first). Builds with an unparseable date are dropped.
pub async fn list_builds(gogdl: Arc<GogDl>, game_id: i32) -> Result<Vec<GameBuild>, GogdlError> {
    let game_builds = gogdl
        .get_game_builds(game_id, OperatingSystem::Windows)
        .await?
        .items;

    let mut dated: Vec<(DateTime<Utc>, GameBuild)> = game_builds
        .into_iter()
        .filter_map(|b| {
            DateTime::parse_from_str(&b.date_published, "%Y-%m-%dT%H:%M:%S%z")
                .ok()
                .map(|dt| (dt.with_timezone(&Utc), b))
        })
        .collect();
    dated.sort_by_key(|(dt, _)| Reverse(*dt));

    Ok(dated.into_iter().map(|(_, b)| b).collect())
}

/// Fetches all builds for a game and returns the one with the most recent
/// `date_published`, or `None` if no build has a parseable publish date.
pub async fn find_latest_build(
    gogdl: Arc<GogDl>,
    game_id: i32,
) -> Result<Option<GameBuild>, GogdlError> {
    Ok(list_builds(gogdl, game_id).await?.into_iter().next())
}

/// Prints all builds for a game (latest first, tagged `(latest)`) for the
/// `download --list-builds` CLI flag.
pub async fn list_builds_cli(gogdl: Arc<GogDl>, game_id: i32) -> Result<(), GogdlError> {
    let builds = list_builds(gogdl, game_id).await?;

    if builds.is_empty() {
        println!("No builds found for this game.");
        return Ok(());
    }

    println!();
    println!("{}", console::style("Available builds:").bold());
    for (idx, build) in builds.iter().enumerate() {
        let tag = if idx == 0 { "  (latest)" } else { "" };
        println!(
            "  {} - {}{}",
            console::style(&build.version_name).cyan(),
            build.date_published,
            console::style(tag).dim()
        );
    }
    println!();

    Ok(())
}

/// Drives `estimate_download` then `download_build` over a single shared
/// progress channel, so the displayed bar climbs live during verification
/// (no silent freeze on resumed downloads) and then continues — without
/// resetting — through the download phase, since `download_build`'s
/// `verified_files` parameter lets it skip re-verifying chunks the estimate
/// pass already confirmed and report progress on that same `0..total_size`
/// scale.
pub async fn download_game(
    gogdl: Arc<GogDl>,
    game_id: i32,
    build_name: &str,
    path: &str,
) -> Result<bool, GogdlError> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(i64, i64)>();
    let (verifying_tx, verifying_rx) = tokio::sync::watch::channel(true);

    let build_name_clone = build_name.to_string();
    let path_clone = path.to_string();
    let tx_for_download = tx.clone();

    let cancellation_token = CancellationToken::new();
    let cancellation_token_for_download = cancellation_token.clone();
    let canceller = spawn_ctrl_c_canceller(cancellation_token);

    let task = tokio::spawn(async move {
        let estimate = gogdl
            .estimate_download(
                game_id,
                OperatingSystem::Windows,
                &build_name_clone,
                &path_clone,
                tx,
            )
            .await?;
        verifying_tx.send(false).ok();
        gogdl
            .download_build(
                game_id,
                &build_name_clone,
                tx_for_download,
                &path_clone,
                OperatingSystem::Windows,
                cancellation_token_for_download,
                DownloadOptions::default(),
                estimate.verified_files,
            )
            .await
    });

    let mut total_size: i64 = 0;
    let mut last_progress: i64 = 0;
    let mut printed_total = false;
    let mut speed_tracker = SpeedTracker::new(Duration::from_secs(1));

    while let Some((progress, total)) = rx.recv().await {
        total_size = total;
        last_progress = progress;

        if !printed_total {
            println!("Total size: {} MB", total_size / 1024 / 1024);
            printed_total = true;
        }

        let verifying = *verifying_rx.borrow();
        if verifying {
            let percent = (progress as f64 / total_size.max(1) as f64) * 100.0;
            print!(
                "\r{} [{:50}] {:.2}%  ({:.2} MB / {:.2} MB)",
                console::style("Verifying:").green(),
                "=".repeat((percent as usize) / 2),
                percent,
                progress as f64 / 1024.0 / 1024.0,
                total_size as f64 / 1024.0 / 1024.0,
            );
        } else {
            let percent = (progress as f64 / total_size.max(1) as f64) * 100.0;
            let speed_suffix = if let Some(speed) = speed_tracker.sample(progress) {
                let remaining_bytes = (total_size - progress).max(0) as f64;
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
                console::style("Downloading:").yellow(),
                "=".repeat((percent as usize) / 2),
                percent,
                progress as f64 / 1024.0 / 1024.0,
                total_size as f64 / 1024.0 / 1024.0,
                speed_suffix
            );
        }

        let _ = std::io::stdout().flush();
    }

    println!(); // New line after progress
    canceller.abort();

    match task.await {
        Ok(Ok(())) => {}
        Ok(Err(GogdlError::ClientError(ClientError::Cancelled))) => {
            println!("Download cancelled.");
            return Ok(false);
        }
        Ok(Err(err)) => {
            println!("Error downloading build: {}", err);
            return Err(err);
        }
        Err(join_err) => {
            println!("Download task failed: {}", join_err);
            return Ok(false);
        }
    }

    if last_progress >= total_size {
        println!("Download complete!");
        Ok(true)
    } else {
        println!(
            "Download incomplete! ({}/{} bytes)",
            last_progress, total_size
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
    let gogdl_for_cleanup = gogdl.clone();

    let cancellation_token = CancellationToken::new();
    let cancellation_token_for_repair = cancellation_token.clone();
    let canceller = spawn_ctrl_c_canceller(cancellation_token);

    let task = tokio::spawn(async move {
        gogdl
            .verify_and_repair_build(
                game_id,
                &build_name_clone,
                tx,
                &path_clone,
                OperatingSystem::Windows,
                cancellation_token_for_repair,
                DownloadOptions::default(),
            )
            .await
    });

    let mut total_size: i64 = 0;
    let mut last_verified: i64 = 0;
    let mut printed_total = false;
    let mut speed_tracker = SpeedTracker::new(Duration::from_secs(1));

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

                let speed_suffix = if let Some(speed) = speed_tracker.sample(downloaded) {
                    format!("  {}/s", format_speed(speed))
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
    canceller.abort();

    let summary = match task.await {
        Ok(Err(GogdlError::ClientError(ClientError::Cancelled))) => {
            println!("Repair cancelled.");
            return Err(GogdlError::ClientError(ClientError::Cancelled));
        }
        Ok(inner) => inner?,
        Err(join_err) => return Err(ClientError::AsyncError(join_err).into()),
    };

    print_repair_summary(&summary);

    // Best-effort: the build itself is already valid by this point, so a cleanup
    // failure (e.g. a permission error deleting a residual file) shouldn't fail
    // the repair/update/change-build call that already succeeded.
    match gogdl_for_cleanup
        .cleanup_build(
            game_id,
            build_name,
            path,
            OperatingSystem::Windows,
            CancellationToken::new(),
        )
        .await
    {
        Ok(cleanup) => print_cleanup_summary(&cleanup),
        Err(err) => println!(
            "{}",
            console::style(format!("⚠ Could not clean up residual files: {}", err)).yellow()
        ),
    }

    Ok(summary)
}

fn print_cleanup_summary(summary: &CleanupSummary) {
    if summary.removed_files.is_empty() && summary.removed_dirs.is_empty() {
        return;
    }

    println!(
        "Cleaned up {} residual file(s) ({} MB) from a previous build.",
        summary.removed_files.len(),
        summary.removed_bytes / 1024 / 1024
    );
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

/// Cancels `token` on the first Ctrl+C and force-exits the process on a
/// second one, in case the in-flight operation hangs during cleanup.
/// Returns the task handle so the caller can `.abort()` it once the guarded
/// operation finishes, instead of leaking a listener for the rest of the process.
fn spawn_ctrl_c_canceller(token: CancellationToken) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_err() {
            return;
        }
        println!(
            "\n{}",
            console::style("Cancelling... (press Ctrl+C again to force quit)").yellow()
        );
        token.cancel();
        if tokio::signal::ctrl_c().await.is_ok() {
            std::process::exit(130); // conventional SIGINT exit code
        }
    })
}

/// Tracks a trailing window of cumulative-byte samples to compute a
/// recent-average transfer speed, so the displayed rate reacts to real
/// throughput changes within the window instead of being smoothed out by
/// the whole transfer's history.
struct SpeedTracker {
    samples: VecDeque<(Instant, i64)>,
    window: Duration,
}

impl SpeedTracker {
    fn new(window: Duration) -> Self {
        Self {
            samples: VecDeque::new(),
            window,
        }
    }

    /// Records a new cumulative-bytes sample and returns the bytes/sec
    /// average over the trailing window, or `None` until enough time has
    /// passed for a stable estimate.
    fn sample(&mut self, bytes: i64) -> Option<f64> {
        let now = Instant::now();
        self.samples.push_back((now, bytes));
        while self.samples.len() > 1 && now.duration_since(self.samples[0].0) > self.window {
            self.samples.pop_front();
        }
        let (oldest_t, oldest_bytes) = *self.samples.front()?;
        let elapsed = now.duration_since(oldest_t).as_secs_f64();
        if elapsed < 0.2 {
            return None;
        }
        Some((bytes - oldest_bytes) as f64 / elapsed)
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
