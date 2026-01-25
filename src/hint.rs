#![allow(dead_code)]

use console::style;

/// Prints a hint showing the CLI command equivalent of an interactive action
pub fn print_command_hint(command: &str) {
    println!();
    println!(
        "{} {}",
        style("💡 CLI equivalent:").dim(),
        style(command).cyan()
    );
    println!();
}

/// Build a CLI command string for running a game
pub fn run_command(game_id: i32) -> String {
    format!("gogdl run -g {}", game_id)
}

/// Build a CLI command string for downloading a game
pub fn download_command(game_id: i32, path: Option<&str>) -> String {
    match path {
        Some(p) => format!("gogdl download -g {} -p \"{}\"", game_id, p),
        None => format!("gogdl download -g {}", game_id),
    }
}

/// Build a CLI command string for setting proton version
pub fn manage_set_proton_command(game_id: i32, version: &str) -> String {
    format!("gogdl manage -g {} set-proton -v \"{}\"", game_id, version)
}

/// Build a CLI command string for setting executable
pub fn manage_set_executable_command(game_id: i32, path: &str) -> String {
    format!("gogdl manage -g {} set-executable -p \"{}\"", game_id, path)
}

/// Build a CLI command string for adding an argument
pub fn manage_add_arg_command(game_id: i32, arg: &str) -> String {
    format!("gogdl manage -g {} add-arg -a \"{}\"", game_id, arg)
}

/// Build a CLI command string for clearing arguments
pub fn manage_clear_args_command(game_id: i32) -> String {
    format!("gogdl manage -g {} clear-args", game_id)
}

/// Build a CLI command string for adding an environment variable
pub fn manage_add_env_command(game_id: i32, key: &str, value: &str) -> String {
    format!(
        "gogdl manage -g {} add-env -k \"{}\" -v \"{}\"",
        game_id, key, value
    )
}

/// Build a CLI command string for clearing environment variables
pub fn manage_clear_env_command(game_id: i32) -> String {
    format!("gogdl manage -g {} clear-env", game_id)
}

/// Build a CLI command string for listing games
pub fn games_list_command() -> String {
    "gogdl games -l".to_string()
}

/// Build a CLI command string for listing proton versions
pub fn proton_list_command(page: i32) -> String {
    if page == 1 {
        "gogdl proton -l".to_string()
    } else {
        format!("gogdl proton -l -p {}", page)
    }
}

/// Build a CLI command string for downloading a proton version
pub fn proton_download_command(version: &str) -> String {
    format!("gogdl proton -d \"{}\"", version)
}

/// Build a CLI command string for listing installed proton versions
pub fn proton_installed_command() -> String {
    "gogdl proton -i".to_string()
}

/// Build a CLI command string for removing a proton version
pub fn proton_remove_command(version: &str) -> String {
    format!("gogdl proton -r \"{}\"", version)
}
