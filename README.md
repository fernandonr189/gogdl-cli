# gogdl-cli

This is a command-line tool for downloading games from Gog. It provides a simple way to login and download games.

## Current state

I started this project because i don't like the idea of too many electron based apps on my system. I really like Heroic Games Launcher, i love playing on linux, so i decided to build my own tool in a language that i like, which is Rust :)

Of course it is still not feature-complete, i'm building this in my free time, that's also part of the reason that i'm starting this as a CLI tool. This will allow me to focus on the core features and functionalities that are necessary for a good experience. Once i'm done with the core features, i'll start working on the GUI version of this tool.

Right now, the tool is in its early stages and is not yet ready for regular use. However, i'm working hard to make it as user-friendly as possible.

I've managed to play games without any issues so far downloading games with this tool and importing them on steam.

## Usage:

- Login to your Gog account:

 This will open a browser window to login to your Gog account.
```
gogdl login 
```
After logging in, your browser window will be redirected to a blank page, check the url and find the string after "code=", copy it (This is your login code)
```
gogdl login -c <code>
```
This will login to your gog account and store the session tokens securely using org.freedesktop.secrets

- Downloading games:

```
gogdl games
```
This will list all the games you have access to in the format {Game ID} - {Game Title}

To download a game, copy the game id obtained previously and run:
```
gogdl download -g <game_id> -p <path>
```
This will download the latest version of the game to the specified path.

- Downloading a specific version of a game

```
gogdl download -g <game_id> -v <version> -p <path>
```
This will download the specified version of the game to the specified path.
Example:
```
gogdl download -g 1234567890 -v '2.31a' -p /path/to/download
```

## Acknowledgements

This project was inspired by the Heroic Games Launcher (https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher.git)

## License
This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
