use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run a game (interactive if no game specified)
    Run {
        #[arg(short, long)]
        /// Game ID to run directly (skips interactive selection)
        game_id: Option<i32>,
    },

    /// Auth commands
    Login {
        #[arg(short, long)]
        /// Login with a code, if false will open a browser to obtain a new code
        code: bool,
        #[arg(default_value = None)]
        /// Login auth code (required for first login)
        login_code: Option<String>,
    },

    /// Manage game configuration (interactive if no options specified)
    Manage {
        #[arg(short, long)]
        /// Game ID to manage directly
        game_id: Option<i32>,

        #[command(subcommand)]
        /// Direct management action (optional)
        action: Option<ManageAction>,
    },

    /// Browse and install games (interactive if no game specified)
    Games {
        #[arg(short, long)]
        /// List all owned games (non-interactive)
        list: bool,
    },

    /// Download a specific game by ID
    Download {
        #[arg(short, long)]
        /// Id of the game to download (will download latest version by default)
        game_id: i32,

        #[arg(short, long)]
        /// Id of the version to download (to download a specific build)
        version_id: Option<String>,

        #[arg(short, long)]
        /// Path to download the game to
        path: Option<String>,

        #[arg(short, long)]
        /// Fix the game
        fix: bool,
    },

    /// Manage Proton/Wine versions (interactive if no options specified)
    Proton {
        #[arg(short, long)]
        /// List available versions for download
        list: bool,

        #[arg(short, long)]
        /// Download a specific version
        download: Option<String>,

        #[arg(short, long, default_value = "1")]
        /// Page number for listing versions
        page: i32,

        #[arg(short, long)]
        /// List installed versions
        installed: bool,

        #[arg(short, long)]
        /// Remove an installed version
        remove: Option<String>,
    },
}

#[derive(Subcommand, Clone)]
pub enum ManageAction {
    /// Set the Proton version for a game
    SetProton {
        #[arg(short, long)]
        /// Proton version to use
        version: String,
    },

    /// Set the executable for a game
    SetExecutable {
        #[arg(short, long)]
        /// Path to the executable (relative to game folder)
        path: String,
    },

    /// Add a launch argument
    AddArg {
        #[arg(short, long)]
        /// Argument to add (without leading dash)
        arg: String,
    },

    /// Clear all launch arguments
    ClearArgs,

    /// Add an environment variable
    AddEnv {
        #[arg(short, long)]
        /// Environment variable name
        key: String,
        #[arg(short, long)]
        /// Environment variable value
        value: String,
    },

    /// Clear all environment variables
    ClearEnv,

    /// Download cloud save files for a game
    #[command(name = "download-save-files")]
    DownloadSaveFiles,

    /// Upload cloud save files for a game
    #[command(name = "upload-save-files")]
    UploadSaveFiles,
}
