# gogdl-cli

This is a command-line tool for downloading and running games from GOG. It provides an interactive way to browse your library, install games, and run them using Proton/Wine.

## Current state

I started this project because I don't like the idea of too many Electron-based apps on my system. I really like Heroic Games Launcher and love playing on Linux, so I decided to build my own tool in Rust.

The tool is still in its early stages but is now functional for downloading, configuring, and running games. It features an interactive CLI that makes it easy to browse your library, manage game settings, and launch games.

## Features

- 🔐 Secure login using `org.freedesktop.secrets`
- 🎮 Interactive game browser with fuzzy search
- 📥 Download games from your GOG library
- 🍷 Automatic Proton/Wine prefix management
- ⚙️ Configure game settings (executable, launch args, environment variables)
- 🚀 Run games with automatic setup prompts

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

### Proton Management

```bash
gogdl proton
```

This opens an interactive menu where you can:
- **Browse & download new versions** - Paginated list of available Proton-GE versions with download sizes, showing which are already installed
- **View installed versions** - See all your currently installed Proton versions and their paths
- **Remove an installed version** - Delete a Proton version (warns if it's in use by any games)

### Direct Download (Advanced)

For scripting or if you know the game ID:

```bash
gogdl download -g <game_id> -p <path>
```

Download a specific version:
```bash
gogdl download -g <game_id> -v '<version>' -p <path>
```

## Example Workflow

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

5. Manage configuration (if needed):
   ```bash
   gogdl manage
   # Select game, adjust settings
   ```

## Acknowledgements

This project was inspired by the [Heroic Games Launcher](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher).

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.