<!-- markdownlint-disable MD033 MD041 MD024 -->
<p align="center">
  <img src="app/native/icons/icon.png" alt="Stache Logo" width="128" height="128">
</p>

<h1 align="center">Stache</h1>

<p align="center">
  <strong>A macOS utility suite with status bar, automation, and desktop enhancements</strong>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#configuration">Configuration</a> •
  <a href="#cli-reference">CLI</a> •
  <a href="#development">Development</a> •
  <a href="#license">License</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/platform-macOS-blue?style=flat-square" alt="Platform">
  <img src="https://img.shields.io/badge/license-MIT-green?style=flat-square" alt="License">
  <img src="https://img.shields.io/badge/rust-2024-orange?style=flat-square" alt="Rust">
  <img src="https://img.shields.io/badge/tauri-2.x-purple?style=flat-square" alt="Tauri">
</p>

---

## Overview

Stache is a **macOS-only** Tauri 2.x desktop application that provides a complete desktop enhancement suite with:

- 📊 **Status Bar** — Customizable menubar with system information widgets (workspaces, media, weather, CPU, battery, clock)
- ⌨️ **Global Keybindings** — Configurable keyboard shortcuts for any action or shell command
- 🎨 **Dynamic Wallpapers** — Automatic wallpaper rotation with blur and rounded corners effects
- 🎵 **Media Controls** — Now playing widget with artwork, playback controls, and track info
- 🔊 **Audio Device Management** — Automatic audio device switching based on configurable priority rules
- 📍 **MenuAnywhere** — Summon any app's menu bar at your cursor position with a keyboard + mouse combo
- 🎵 **noTunes** — Prevent Apple Music from auto-launching and optionally open your preferred music app
- ⏹️ **Hold-to-Quit** — Require holding Cmd+Q to quit apps, preventing accidental closes
- 😴 **Keep Awake** — Prevent system sleep with a single click from the status bar
- 🖥️ **Tiling WM Integration** — Built-in support for Hyprspace/yabai/aerospace workspace events

Built with **Rust** for the backend and **React 19** for the frontend, Stache combines native performance with a modern, reactive UI.

---

## Features

### 📊 Status Bar

A sleek, transparent menubar that displays:

| Widget          | Description                                     |
| --------------- | ----------------------------------------------- |
| **Workspaces**  | Visual workspace indicator with click-to-switch |
| **Current App** | Active application name and icon                |
| **Media**       | Now playing track with playback controls        |
| **Weather**     | Current conditions and temperature              |
| **CPU**         | Real-time CPU usage monitor                     |
| **Battery**     | Battery level and charging status               |
| **Keep Awake**  | Prevent system sleep toggle                     |
| **Clock**       | Current time and date                           |

### ⌨️ Global Keybindings

Define custom keyboard shortcuts to:

- Switch workspaces and layouts
- Move and resize windows
- Execute shell commands
- Trigger application actions

### 🎨 Dynamic Wallpapers

- Automatic wallpaper rotation (random or sequential)
- Configurable change interval
- Rounded corners and blur effects
- Per-screen wallpaper support
- Pre-generation for instant switching

### 🔊 Audio Device Management

- Automatic switching when devices connect/disconnect
- Priority-based device selection (e.g., prefer AirPods over built-in speakers)
- Separate input/output device priorities
- Regex and pattern matching for device names
- Device dependency rules (e.g., use speakers only when audio interface is connected)

### 📍 MenuAnywhere

Summon any application's menu bar right at your cursor:

- Configurable modifier keys (Control, Option, Command, Shift)
- Right-click or middle-click trigger
- Works with any macOS application

### 🎵 noTunes

Prevent Apple Music from hijacking your media keys:

- Blocks Apple Music/iTunes from auto-launching
- Optionally launches your preferred music app (Spotify, Tidal) instead
- Works with Bluetooth headphone connections and media key presses

### ⏹️ Hold-to-Quit

Prevent accidental app closures:

- Requires holding Cmd+Q instead of just pressing it
- Visual feedback showing hold progress
- Per-app customization (coming soon)

---

## Installation

### Requirements

- **macOS 10.15** (Catalina) or later

### Download

Download the latest release from the [Releases](https://github.com/marcosmoura/stache/releases) page.

### Build from Source

1. **Install dependencies:**

   ```bash
   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # Install pnpm
   npm install -g pnpm

   # Install project dependencies
   pnpm install
   ```

2. **Build the application:**

   ```bash
   pnpm release
   ```

3. **Install the CLI (optional):**

   ```bash
   pnpm build:cli
   # Binary will be at target/release/stache
   ```

---

## Configuration

Stache uses a JSONC configuration file located at:

`~/.config/stache/config.json`

> **Tip:** JSONC supports comments! Use `//` for single-line and `/* */` for multi-line comments.

### JSON Schema

A JSON Schema is provided for editor autocompletion and validation:

```jsonc
{
  "$schema": "https://raw.githubusercontent.com/marcosmoura/stache/main/stache.schema.json",
  // Your configuration here...
}
```

### Example Configuration

```jsonc
{
  "$schema": "https://raw.githubusercontent.com/marcosmoura/stache/main/stache.schema.json",

  // Status bar configuration
  "bar": {
    "wallpapers": {
      "path": "~/Pictures/Wallpapers",
      "interval": 300,
      "mode": "random",
      "radius": 12,
      "blur": 8,
    },
    "weather": {
      "visualCrossingApiKey": "YOUR_API_KEY",
      "defaultLocation": "San Francisco",
    },
  },

  // Global keybindings
  "keybindings": {
    "Command+Control+R": "stache reload",
  },
}
```

### Configuration Reference

<details>
<summary><strong>Bar Configuration</strong></summary>

#### Wallpapers

| Option     | Type                         | Default    | Description                                            |
| ---------- | ---------------------------- | ---------- | ------------------------------------------------------ |
| `path`     | `string`                     | `""`       | Directory containing wallpaper images                  |
| `list`     | `string[]`                   | `[]`       | Explicit list of wallpaper paths                       |
| `interval` | `number`                     | `0`        | Seconds between wallpaper changes (0 = no auto-change) |
| `mode`     | `"random"` \| `"sequential"` | `"random"` | Wallpaper selection mode                               |
| `radius`   | `number`                     | `0`        | Corner radius in pixels                                |
| `blur`     | `number`                     | `0`        | Gaussian blur amount in pixels                         |

#### Weather

| Option                 | Type     | Default | Description                                                        |
| ---------------------- | -------- | ------- | ------------------------------------------------------------------ |
| `visualCrossingApiKey` | `string` | `""`    | API key from [visualcrossing.com](https://www.visualcrossing.com/) |
| `defaultLocation`      | `string` | `""`    | Fallback location when geolocation fails                           |

</details>

---

## CLI Reference

Stache includes a powerful CLI for scripting and automation.

### Installation

The CLI binary (`stache`) communicates with the running desktop app via distributed notifications.

```bash
# Build the CLI
cargo build --package stache --release
```

### Shell Completions

```bash
# Zsh (add to ~/.zshrc)
eval "$(stache completions --shell zsh)"

# Bash
stache completions --shell bash > ~/.local/share/bash-completion/completions/stache

# Fish
stache completions --shell fish > ~/.config/fish/completions/stache.fish
```

### Commands

#### General

| Command                              | Description                          |
| ------------------------------------ | ------------------------------------ |
| `stache reload`                      | Reload configuration without restart |
| `stache schema`                      | Output JSON schema to stdout         |
| `stache completions --shell <SHELL>` | Generate shell completions           |

#### Wallpaper Management

```bash
# Set specific wallpaper
stache wallpaper set /path/to/image.jpg

# Set random wallpaper
stache wallpaper set --random

# Target specific screen
stache wallpaper set --random --screen main
stache wallpaper set /path/to/image.jpg --screen 2

# Pre-generate all wallpapers
stache wallpaper generate-all

# List available wallpapers
stache wallpaper list
```

---

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (2024 edition)
- [Node.js](https://nodejs.org/) 20+
- [pnpm](https://pnpm.io/) 9+

### Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/)
- [Tauri VS Code extension](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

### Project Structure

```text
stache/
├── app/
│   ├── native/               # Rust backend (Tauri)
│   │   └── src/
│   │       ├── main.rs           # Entry point (CLI + desktop)
│   │       ├── lib.rs            # Tauri app initialization
│   │       ├── cli/              # CLI commands
│   │       ├── config/           # Configuration types
│   │       ├── bar/              # Status bar components
│   │       ├── wallpaper/        # Wallpaper management
│   │       ├── audio/            # Audio device management
│   │       └── utils/            # Utilities (IPC, paths, etc.)
│   │
│   └── ui/                   # React frontend
│       ├── main.tsx              # App entry
│       ├── renderer/             # Window renderers (bar, widgets)
│       ├── components/           # Shared UI components
│       ├── hooks/                # React hooks
│       ├── stores/               # Zustand stores
│       └── design-system/        # Styling tokens
│
├── scripts/                  # Build & release scripts
├── stache.schema.json        # JSON Schema for config
└── Cargo.toml                # Workspace root
```

### Available Scripts

| Command            | Description                           |
| ------------------ | ------------------------------------- |
| `pnpm dev`         | Start Vite dev server (frontend only) |
| `pnpm tauri:dev`   | Full app with hot reload              |
| `pnpm tauri:build` | Build production app                  |
| `pnpm build:cli`   | Build CLI binary                      |
| `pnpm test`        | Run all tests                         |
| `pnpm test:ui`     | Run Vitest browser tests              |
| `pnpm test:rust`   | Run Rust tests with nextest           |
| `pnpm lint`        | Run all linters                       |
| `pnpm format`      | Format all code                       |

### Architecture

```text
┌─────────────┐  NSDistributed      ┌──────────────────────────────────────┐
│   CLI       │  Notification       │         Desktop App                  │
│  (stache)   │ ──────────────────► │  ┌─────────────┐   ┌───────────────┐ │
└─────────────┘                     │  │IPC Listener │──►│ Tauri Events  │ │
                                    │  └─────────────┘   └───────┬───────┘ │
                                    │                            │         │
                                    │  ┌─────────────────────────▼───────┐ │
                                    │  │        React Frontend           │ │
                                    │  │  (React Query + Tauri Invoke)   │ │
                                    │  └─────────────────────────────────┘ │
                                    └──────────────────────────────────────┘
```

### Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run linting and tests (`pnpm lint && pnpm test`)
5. Commit your changes (`git commit -m 'Add amazing feature'`)
6. Push to the branch (`git push origin feature/amazing-feature`)
7. Open a Pull Request

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

<p align="center">
  Made with ❤️ by <a href="https://github.com/marcosmoura">Marcos Moura</a>
</p>
