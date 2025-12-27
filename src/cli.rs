use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Auth commands
    Login {
        #[arg(short, long)]
        /// Login with a code, if false will open a browser to obtain a new code
        code: bool,
        #[arg(default_value = None)]
        /// Login auth code (required for first login)
        login_code: Option<String>,
    },

    /// See games info
    Games,

    /// Download a game
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
    /// List wine versions
    Proton {
        #[arg(short, long)]
        /// List wine versions
        list: bool,

        #[arg(short, long)]
        /// Download a specific wine version
        download: Option<String>,

        #[arg(short, long, default_value = "1")]
        /// Page number to list wine versions
        page: i32,
    },
}
