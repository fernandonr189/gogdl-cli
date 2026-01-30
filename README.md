# gogdl-cli

This is a command-line tool for downloading and running games from GOG. It provides both an interactive mode for ease of use and direct CLI commands for scripting and automation.

## Current state

I started this project because I don't like the idea of too many Electron-based apps on my system. I really like Heroic Games Launcher and love playing on Linux, so I decided to build my own tool in Rust.

The tool is now functional for downloading, configuring, running games, and managing cloud saves. It features an interactive CLI that makes it easy to browse your library, manage game settings, launch games, and download cloud saves - while also providing direct CLI commands for scripting.

## Features

- 🔐 Secure login using `org.freedesktop.secrets`
- 🎮 Interactive game browser with fuzzy search
- 📥 Download games from your GOG library
- 🍷 Automatic Proton/Wine prefix management
- ⚙️ Configure game settings (executable, launch args, environment variables)
- 🚀 Run games with automatic setup prompts
- ☁️ Download cloud save files for games that support them
- 💡 CLI command hints shown during interactive use (for learning scripting commands)

## Installation

```bash
cargo install --path .
```

## Usage

### Login

First, login to your GOG account:

```bash
gogdl login
```

This will open a browser window to login to your GOG account. After logging in, your browser will be redirected to a blank page. Copy the code from the URL (the string after `code=`) and run:

```bash
gogdl login -c <code>
```

Your session tokens will be stored securely using `org.freedesktop.secrets`.

---

## Interactive Mode

The following commands open interactive menus when run without arguments:

### Browse and Install Games

```bash
gogdl games
```

This opens an interactive menu where you can:
- Browse your GOG library using arrow keys
- Search for games by typing (fuzzy search)
- See which games are already installed
- Select a game to install
- Choose a custom install path or use the default

### Run Games

```bash
gogdl run
```

This opens an interactive menu to:
- Select from your installed games
- See which games are ready to run and which need configuration
- If a game hasn't been configured yet, you'll be prompted to:
  - Select a Proton version (from your downloaded versions)
  - Select the game executable (auto-detected from the game folder)
- Automatically creates Wine prefixes on first run
- Saves your configuration for future launches

### Manage Game Configuration

```bash
gogdl manage
```

This opens an interactive menu where you can:
- Select a game to configure
- Set/change the Proton version
- Set/change the game executable
- Add launch arguments
- Add environment variables
- Clear arguments or environment variables
- Download cloud save files (for games that support cloud saves)
- Upload cloud save files (for games that support cloud saves)

### Proton Management

```bash
gogdl proton
```

This opens an interactive menu where you can:
- **Browse & download new versions** - Paginated list of available Proton-GE versions with download sizes
- **View installed versions** - See all your currently installed Proton versions
- **Remove an installed version** - Delete a Proton version (warns if it's in use)

---

## Direct CLI Commands (for scripting)

All commands also support direct CLI flags for use in scripts or automation.

### List Owned Games

```bash
gogdl games -l
```

Lists all games in your GOG library with their IDs.

### Download a Game

```bash
# Download to default location
gogdl download -g <game_id>

# Download to custom path
gogdl download -g <game_id> -p "/path/to/games"

# Download specific version
gogdl download -g <game_id> -v "<version>"

# Re-download/fix existing game
gogdl download -g <game_id> -f
```

### Run a Game

```bash
gogdl run -g <game_id>
```

Runs the specified game directly (must be configured first).

### Manage Game Configuration

```bash
# Set Proton version
gogdl manage -g <game_id> set-proton -v "<version>"

# Set executable
gogdl manage -g <game_id> set-executable -p "path/to/game.exe"

# Add launch argument
gogdl manage -g <game_id> add-arg -a "windowed"

# Clear all launch arguments
gogdl manage -g <game_id> clear-args

# Add environment variable
gogdl manage -g <game_id> add-env -k "VARIABLE" -v "value"

# Clear all environment variables
gogdl manage -g <game_id> clear-env

# Download cloud save files
gogdl manage -g <game_id> download-save-files

# Upload cloud save files
gogdl manage -g <game_id> upload-save-files
```

### Proton Management

```bash
# List available versions (paginated)
gogdl proton -l
gogdl proton -l -p 2  # Page 2

# Download a specific version
gogdl proton -d "GE-Proton9-20"

# List installed versions
gogdl proton -i

# Remove an installed version
gogdl proton -r "GE-Proton9-20"
```

---

## Example Workflows

### Interactive Workflow

1. Login to GOG:
   ```bash
   gogdl login
   # Open browser, copy code
   gogdl login -c YOUR_CODE
   ```

2. Download a Proton version:
   ```bash
   gogdl proton
   # Select "Browse & download new versions"
   # Navigate with arrows, select a version to download
   ```

3. Browse and install a game:
   ```bash
   gogdl games
   # Navigate with arrows, type to search, select game, choose path
   ```

4. Run the game:
   ```bash
   gogdl run
   # Select game, choose Proton version if prompted, choose executable if prompted
   ```

### Scripted Workflow

```bash
#!/bin/bash

# Download Proton
gogdl proton -d "GE-Proton9-20"

# Download a game (replace with actual game ID)
gogdl download -g 1234567890

# Configure the game
gogdl manage -g 1234567890 set-proton -v "GE-Proton9-20"
gogdl manage -g 1234567890 set-executable -p "Game.exe" 
gogdl manage -g 1234567890 add-env -k "DXVK_HUD" -v "fps"

# Download cloud saves if the game supports them
gogdl manage -g 1234567890 download-save-files

# Run the game
gogdl run -g 1234567890
```

---

## CLI Command Hints

When using interactive mode, the tool displays the equivalent CLI command before performing each action. This helps you learn the CLI commands for future scripting:

```
💡 CLI equivalent: gogdl download -g 1234567890 -p "/home/user/.local/share/gogdl/games"
```

---

## Acknowledgements

This project was inspired by the [Heroic Games Launcher](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher).

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
