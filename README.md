# rfm
Console File Manager for Linux on Rust
Консольный файловый менеджер для Lunux на языке Rust
<img width="1562" height="794" alt="070110" src="https://github.com/user-attachments/assets/198669c6-6ebd-44f4-889a-5d8c23fcfb27" />
# RFM — Rust File Manager

A terminal-based dual-panel file manager inspired by Total Commander, written in Rust.

## Description

RFM runs in any terminal on any Linux distribution.

**Features:**
- Two independent panels: left and right
- Color scheme: directories — blue, files — green, selected — yellow
- Fully configurable hotkeys via `~/.config/rfm/keymap.toml`
- File openers configured per extension via `~/.config/rfm/openers.toml`
- USB drive menu with automatic mount point detection
- File search with forward/backward navigation
- Permissions editor (chmod) with sudo support and recursive mode
- Progress bar during copy and move operations

## Build

```bash
cargo build --release
```

## Install

```bash
sudo cp target/release/rfm /usr/local/bin/rfm
```

## Launch

```bash
rfm
```

## Configuration

Configuration files are located in `~/.config/rfm/`

| File | Description |
|------|-------------|
| `keymap.toml` | Key bindings |
| `openers.toml` | Programs to open files by extension (assigned from the app — `Alt+Enter`) |

Example `openers.toml`:
```toml
[openers]
pdf  = "evince"
mp4  = "mpv"
txt  = "kate"
png  = "eog"
zip  = "ark"
```

## Hotkeys

### Navigation

| Key | Action |
|-----|--------|
| `↑` / `↓` | Move cursor |
| `Alt+↑` | Jump up (default: 5 lines) |
| `Alt+↓` | Jump down (default: 5 lines) |
| `PageUp` / `PageDown` | Scroll one page up/down |
| `g` | Go to the top of the list |
| `G` | Go to the bottom of the list |
| `Enter` / `→` | Enter directory / open file |
| `Backspace` / `←` | Go up one directory level |
| `\` | Go to root directory `/` |

### Panels

| Key | Action |
|-----|--------|
| `Tab` | Switch between panels |
| `[` | Focus left panel |
| `]` | Focus right panel |

### File Operations

| Key | Action |
|-----|--------|
| `Space` | Select / deselect file |
| `F5` | Copy selected files to the other panel |
| `F6` | Move selected files to the other panel |
| `F7` | Create new directory |
| `F8` | Delete selected files (with confirmation) |
| `F2` | Create new file |
| `r` | Refresh both panels |

### Search

| Key | Action |
|-----|--------|
| `F3` | Open search panel |
| `Enter` | Find next match |
| `Esc` | Close search |

### Permissions

| Key | Action |
|-----|--------|
| `Alt+f` | Open permissions dialog (chmod) |
| `Space` | Toggle checkbox |
| `Enter` | Apply (uses sudo if needed) |

### USB Drives

| Key | Action |
|-----|--------|
| `Alt+u` | Open USB drives menu |
| `Enter` | Navigate to mount point |
| `Esc` | Close menu |

### Other

| Key | Action |
|-----|--------|
| `Alt+Enter` | Set or change the opener for the file's extension |
| `q` | Quit the program |

## Keymap Configuration

Example `~/.config/rfm/keymap.toml`:

```toml
[keys]
quit          = "q"
panel_switch  = "Tab"
panel_left    = "["
panel_right   = "]"
go_up         = "Backspace"
enter         = "Enter"
jump_up       = "Alt+Up"
jump_down     = "Alt+Down"
jump_amount   = 5
go_top        = "g"
go_bottom     = "G"
page_up       = "PageUp"
page_down     = "PageDown"
select        = "Space"
copy          = "F5"
move_files    = "F6"
mkdir         = "F7"
delete        = "F8"
create_file   = "F2"
search        = "F3"
chmod         = "Alt+f"
usb_menu      = "Alt+u"
refresh       = "r"
go_root       = "\\"
```

Supported key names: `Enter`, `Tab`, `Backspace`, `Esc`, `Space`, `Delete`,
`Up`, `Down`, `Left`, `Right`, `Home`, `End`, `PageUp`, `PageDown`,
`F1`–`F10`, `Alt+X`, `Ctrl+X`, or any single character.
