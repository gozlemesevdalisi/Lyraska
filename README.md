# Lyraska

> **Early development** — Phase 1 is in progress. Lyraska can already open and play local audio
> files on Windows (MP3, FLAC, WAV, AIFF, OGG Vorbis, AAC/ALAC); visuals and the library come next.

An offline music player for Windows whose visuals **know the song before it plays**.

The name comes from the _lyre_ and the _Lyra_ constellation.

Every track is analysed ahead of time — beats, bars, sections, drops and energy — producing a
_song map_. A **Visual Director** then choreographs the visuals in three layers (**atmosphere**,
**rhythm** and **texture**) so that scenes anticipate the music instead of merely reacting to it.

Alongside the visuals, Lyraska aims to be a serious audio player: parametric EQ, AutoEq
headphone profiles, EBU R128 loudness normalisation, crossfeed and a bit-perfect output mode.

## Highlights (planned)

- **Fully offline** — no accounts, no telemetry, no network required.
- **Song-aware visuals** — WebGL2 scenes driven by pre-computed analysis, with latency
  compensation targeting ±20 ms audio/visual sync.
- **Nostalgia modes** — original designs inspired by classic car-stereo displays and VU meters.
  No third-party brand names or assets are used.
- **Photosensitivity-safe** — a safe mode caps flashes at three per second, and the app honours
  the system "reduce motion" preference.
- **Pro audio** — 64-bit internal processing, WASAPI output, parametric EQ, AutoEq, loudness,
  crossfeed, bit-perfect mode.

See the [roadmap](docs/ROADMAP.md) (in Turkish) for phases and dates. Version 1.0 is planned for
early March 2027.

## Download

The latest test build of the Windows installer is always available at the same link:

**<https://github.com/gozlemesevdalisi/Lyraska/releases/download/test-surumu/Lyraska-Kurulum.exe>**

It is rebuilt automatically every time `main` changes. Builds are not code-signed yet, so Windows
SmartScreen will show an "unknown publisher" warning: click **More info → Run anyway**.

## Tech stack

| Area     | Technology                                                   |
| -------- | ------------------------------------------------------------ |
| Shell    | [Tauri 2](https://tauri.app)                                 |
| Audio    | Rust — `symphonia` (decoding), `wasapi` (output), 64-bit DSP |
| Analysis | Rust                                                         |
| UI       | React + TypeScript + Vite                                    |
| Visuals  | WebGL2                                                       |
| Storage  | SQLite                                                       |

Windows comes first; macOS and Linux are planned after 1.0.

## Building from source

Requirements: [Node.js 22](https://nodejs.org), [Rust (stable)](https://rustup.rs) and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for your platform.

```sh
npm ci
npm run tauri dev     # run the app in development mode
npm run tauri build   # build the installer
```

Checks:

```sh
npm run lint && npm run typecheck && npm test
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## Project layout

```
src/          React + TypeScript UI
src-tauri/    Rust core: audio engine, analysis, visual bridge
docs/         Roadmap and development log (Turkish)
```

## Contributing & bug reports

Bug reports are welcome via the issue form (in Turkish). The primary language of the project is
Turkish; code identifiers are in English.

## License

[GPL-3.0-only](LICENSE). Dependencies licensed under the AGPL or under terms that forbid
commercial use are not accepted; this is enforced in CI.
