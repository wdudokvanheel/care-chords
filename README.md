# Care Chords

Care Chords seamlessly blends an IP camera’s audio with Spotify, allowing you to enjoy music while
keeping an ear on your little ones.

## Features

- **AI-powered noise filtering** – Eliminates static and background noise from the camera’s audio
- **Full Spotify control** – Effortlessly browse and select playlists with automatic filtering
- **Smart sleep timer** – Gradually fades out music while keeping the baby monitor audio active
- **Live, low-latency video** – View the camera feed in real time when the app is open
- **Background playback** – Keep monitoring and listening to music even when the app is minimized
- **Auto-mute on device change** – Instantly mutes when output devices switch

![Project stats](https://pstatool.wdudokvanheel.nl/wdudokvanheel/care-chords.svg)

## Screenshots

| Now Playing                          | Playlist Selector                                |
|--------------------------------------|--------------------------------------------------|
| ![Now Playing](docs/now_playing.png) | ![Playlist Selector](docs/playlist_selector.png) |

## How does it work

The Rust backend uses GStreamer to capture audio from an IP camera via RTSP and combines this with
the integrated Spotify client. The streams are processed, mixed, and streamed to the clients.

The client communicates with the backend over HTTP to control playback, song selection, the sleep timer, and other settings.

## To Do

- [ ] Add configurable connection settings (currently, all addresses are hardcoded)
- [ ] Implement a settings menu

### Docker compose with config file

```
version: '3.8'

services:
  carechords:
    image: carechords:latest
    container_name: carechords
    restart: unless-stopped
    network_mode: "host"
    volumes:
      - ./carechords.toml:/etc/carechords.toml
      - ./spotify_cache:/cache

```

### Docker compose with environment variables

```
version: '3.8'

services:
  carechords:
    image: carechords:latest
    container_name: carechords
    restart: unless-stopped
    network_mode: "host"
    environment:
        CARECHORDS_RTSP_PORT: 8554
        CARECHORDS_MONITOR_URL: "rtsp://sleepstream:sleepstream@10.0.0.51"
        CARECHORDS_NOISE_FILTER: true
    volumes:
      - ./spotify_cache:/cache
```

