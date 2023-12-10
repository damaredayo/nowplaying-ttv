# nowplaying-ttv

## An open source and cross-platform application for easily displaying the currently playing song for your livestream!

nowplaying-ttv aims to be a simple, configurable and as much possible easy to use application for displaying the currently playing song for your livestream. It is written in Rust.

## !! THE PROJECT IS IN REALLY EARLY STAGES OF DEVELOPMENT AND IS VERY BUGGY !! 

Below you can find all of the currently supported features and the ones that are planned to be implemented. If you have any suggestions or ideas, please feel free to open an issue or a pull request. I would love to see them! :)

## Features / Todolist

- [x] Twitch Chat integration
- [x] Twitch OAuth integration
- [x] Soundcloud API v2 integration
- [x] Spotify API integration
- [x] Spotify OAuth integration
- [x] Web dashboard for settings (but very rudimentary)

## Environment variables

nowplaying-ttv can be configured using environment variables. The following environment variables are supported:

| Variable name | Description | Default value | Optional |
| ------------- | ----------- | ------------- | -------- |
| `TWITCH_CLIENT_ID` | The Twitch client ID to use for the Twitch API | `None` | ❌
| `TWITCH_CLIENT_SECRET` | The Twitch client secret to use for the Twitch API | `None` | ❌
| `TWITCH_USERNAME` | The Twitch Username of the chat to join | `None` | ❌
| `SOUNDCLOUD_ENABLED` | Whether to enable Soundcloud integration | `false` | ✔️
| `SOUNDCLOUD_OAUTH` | The Soundcloud OAuth token to use for the Soundcloud API (v2) | `None` | ✔️
| `SPOTIFY_ENABLED` | Whether to enable Spotify integration | `false` | ✔️
| `SPOTIFY_CLIENT_ID` | The Spotify client ID to use for the Spotify API | `None` | ✔️
| `SPOTIFY_CLIENT_SECRET` | The Spotify client secret to use for the Spotify API | `None` | ✔️
| `CONFIG_FILE` | The path to the configuration file to use | `~/.config/nowplaying-ttv/config.json`* | ✔️

*This is `~/.config/nowplaying-ttv/config.json` on Linux and `%APPDATA%\nowplaying-ttv\config.json` on Windows.

## Configuration file

nowplaying-ttv can be configured using a JSON file. The default configuration file is located in `~/.config/nowplaying-ttv/config.json` on Linux and `%APPDATA%\nowplaying-ttv\config.json` on Windows. You can also specify a custom configuration file by using the `-c` flag or using the `CONFIG_FILE` environment variable.

The configuration file is structured as follows:

```json
{
    "twitch_client_id": "fsujv7qqhgv9u3xxxxxxxxxxxxxxxx",
    "twitch_client_secret": "o47fs3x6e1ni7xxxxxxxxxxxxxxxx",
    "twitch_username": "dhopcs",
    "soundcloud_enabled": true,
    "soundcloud_oauth": "OAuth 2-123456-123456789-xxxxxxxxxxxxxx",
    "spotify_enabled": true,
    "spotify_client_id": "38a53b04205fd6a982xxxxxxxxxxxxxx",
    "spotify_client_secret": "382b0ec90fb3420bxxxxxxxxxxxxxxxx"
}
```

## Installing

### Prebuilt binaries

There are builds of the application for Windows and Linux. You can find them in the [releases](https://github.com/damaredayo/nowplaying-ttv/releases) page. Note that the Linux builds are not tested on all distributions, so if you encounter any issues, please open an issue or a pull request. :)

### Docker

There is a docker image on [Docker Hub](https://hub.docker.com/r/damaredayo/nowplaying-ttv).
Ensure you have a functioning `config.json` file, or set the correct environment variables.

You can run it by using the following command:

```bash
docker run -d -p 8080:8080 -v /path/to/config.json:/root/.config/nowplaying-ttv/config.json damaredayo/nowplaying-ttv \
    -e TWITCH_CLIENT_ID=fsujv7qqhgv9u3xxxxxxxxxxxxxxxx \
    -e TWITCH_CLIENT_SECRET=o47fs3x6e1ni7xxxxxxxxxxxxxxxx \
    -e TWITCH_USERNAME=dhopcs \
    -e SOUNDCLOUD_ENABLED=true \
    -e SOUNDCLOUD_OAUTH=OAuth 2-123456-123456789-xxxxxxxxxxxxxx
```

or a handy docker-compose file:

```yaml
version: '3.7'

services:
  nowplaying-ttv:
    image: damaredayo/nowplaying-ttv
    container_name: nowplaying-ttv
    restart: unless-stopped
    ports:
      - 9090:9090
    volumes:
      - /path/to/config.json:/root/.config/nowplaying-ttv/config.json
    environment:
      - TWITCH_CLIENT_ID=fsijv3qqhgv9u3xxxxxxxxxxxxxxxx
      - TWITCH_CLIENT_SECRET=o47fs3x6e1ni7xxxxxxxxxxxxxxxx
      - TWITCH_USERNAME=dhopcs
      - SOUNDCLOUD_ENABLED=true
      - SOUNDCLOUD_OAUTH=OAuth 2-123456-123456789-xxxxxxxxxxxxxx
      - INTERNAL_PORT=8080
    args:
      - PORT=9090
      - INTERNAL_PORT=8080
```

## Building

nowplaying-ttv is written in Rust, so you will need to have Rust installed in order to build it. You can get it from [here](https://rustup.rs/).

After you have Rust installed, you can clone the repository and build the project by running the following command in the root directory of the project:

```bash
cargo build --release
```

Upon buidling, the binary will be located in `target/release/nowplaying-ttv`.


## Usage

Once configured and running, you can access the web dashboard by going to `http://localhost:8080` in your browser. You can also specify a custom port by using the `-p` flag.