use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

// ─── Keymap ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct Keys {
    #[serde(default = "d_quit")]          pub quit:         String,
    #[serde(default = "d_panel_switch")]  pub panel_switch: String,
    #[serde(default = "d_go_up")]         pub go_up:        String,
    #[serde(default = "d_enter")]         pub enter:        String,
    #[serde(default = "d_jump_down")]     pub jump_down:    String,
    #[serde(default = "d_jump_up")]       pub jump_up:      String,
    #[serde(default = "d_jump_amount")]   pub jump_amount:  usize,
    #[serde(default = "d_select")]        pub select:       String,
    #[serde(default = "d_copy")]          pub copy:         String,
    #[serde(default = "d_move_files")]    pub move_files:   String,
    #[serde(default = "d_mkdir")]         pub mkdir:        String,
    #[serde(default = "d_delete")]        pub delete:       String,
    #[serde(default = "d_go_top")]        pub go_top:       String,
    #[serde(default = "d_go_bottom")]     pub go_bottom:    String,
    #[serde(default = "d_page_up")]       pub page_up:      String,
    #[serde(default = "d_page_down")]     pub page_down:    String,
    #[serde(default = "d_refresh")]       pub refresh:      String,
    #[serde(default = "d_go_root")]       pub go_root:      String,
    #[serde(default = "d_create_file")]   pub create_file:  String,
    #[serde(default = "d_chmod")]         pub chmod:        String,
    #[serde(default = "d_search")]        pub search:       String,
    #[serde(default = "d_panel_left")]    pub panel_left:  String,
    #[serde(default = "d_panel_right")]   pub panel_right: String,
    #[serde(default = "d_usb_menu")]      pub usb_menu:    String,
}

fn d_quit()         -> String { "q".into() }
fn d_panel_switch() -> String { "Tab".into() }
fn d_go_up()        -> String { "Backspace".into() }
fn d_enter()        -> String { "Enter".into() }
fn d_jump_down()    -> String { "Alt+Down".into() }
fn d_jump_up()      -> String { "Alt+Up".into() }
fn d_jump_amount()  -> usize  { 5 }
fn d_select()       -> String { "Space".into() }
fn d_copy()         -> String { "F5".into() }
fn d_move_files()   -> String { "F6".into() }
fn d_mkdir()        -> String { "F7".into() }
fn d_delete()       -> String { "F8".into() }
fn d_go_top()       -> String { "g".into() }
fn d_go_bottom()    -> String { "G".into() }
fn d_page_up()      -> String { "PageUp".into() }
fn d_page_down()    -> String { "PageDown".into() }
fn d_refresh()      -> String { "r".into() }
fn d_go_root()      -> String { "\\".into() }
fn d_create_file()  -> String { "F2".into() }
fn d_chmod()        -> String { "Alt+f".into() }
fn d_search()       -> String { "F3".into() }
fn d_panel_left()   -> String { "[".into() }
fn d_panel_right()  -> String { "]".into() }
fn d_usb_menu()     -> String { "Alt+u".into() }

impl Default for Keys {
    fn default() -> Self {
        Self {
            quit: d_quit(), panel_switch: d_panel_switch(), go_up: d_go_up(),
            enter: d_enter(), jump_down: d_jump_down(), jump_up: d_jump_up(),
            jump_amount: d_jump_amount(), select: d_select(), copy: d_copy(),
            move_files: d_move_files(), mkdir: d_mkdir(), delete: d_delete(),
            go_top: d_go_top(), go_bottom: d_go_bottom(),
            page_up: d_page_up(), page_down: d_page_down(), refresh: d_refresh(),
            go_root: d_go_root(), create_file: d_create_file(),
            chmod: d_chmod(), search: d_search(),
            panel_left: d_panel_left(), panel_right: d_panel_right(),
            usb_menu: d_usb_menu(),
        }
    }
}

// ─── Openers ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
struct OpenersFile {
    #[serde(default)]
    openers: HashMap<String, String>,
}

// ─── Load functions ──────────────────────────────────────────────────────────

pub fn load_keymap() -> Keys {
    #[derive(Deserialize, Default)]
    struct KeymapFile {
        #[serde(default)]
        keys: Keys,
    }

    let path = config_dir().join("keymap.toml");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str::<KeymapFile>(&s).ok())
        .map(|f| f.keys)
        .unwrap_or_default()
}

pub fn load_openers() -> HashMap<String, String> {
    let path = config_dir().join("openers.toml");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str::<OpenersFile>(&s).ok())
        .map(|f| f.openers)
        .unwrap_or_default()
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rfm")
}

// ─── Key parsing ─────────────────────────────────────────────────────────────

pub fn parse_key(s: &str) -> Option<KeyEvent> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("Alt+") {
        return Some(KeyEvent::new(parse_code(rest)?, KeyModifiers::ALT));
    }
    if let Some(rest) = s.strip_prefix("Ctrl+") {
        return Some(KeyEvent::new(parse_code(rest)?, KeyModifiers::CONTROL));
    }
    if let Some(rest) = s.strip_prefix("Shift+") {
        return Some(KeyEvent::new(parse_code(rest)?, KeyModifiers::SHIFT));
    }
    Some(KeyEvent::new(parse_code(s)?, KeyModifiers::NONE))
}

fn parse_code(s: &str) -> Option<KeyCode> {
    Some(match s {
        "Enter"     => KeyCode::Enter,
        "Tab"       => KeyCode::Tab,
        "Backspace" => KeyCode::Backspace,
        "Delete"    => KeyCode::Delete,
        "Esc"       => KeyCode::Esc,
        "Up"        => KeyCode::Up,
        "Down"      => KeyCode::Down,
        "Left"      => KeyCode::Left,
        "Right"     => KeyCode::Right,
        "Home"      => KeyCode::Home,
        "End"       => KeyCode::End,
        "PageUp"    => KeyCode::PageUp,
        "PageDown"  => KeyCode::PageDown,
        "Space"     => KeyCode::Char(' '),
        "F1"        => KeyCode::F(1),
        "F2"        => KeyCode::F(2),
        "F3"        => KeyCode::F(3),
        "F4"        => KeyCode::F(4),
        "F5"        => KeyCode::F(5),
        "F6"        => KeyCode::F(6),
        "F7"        => KeyCode::F(7),
        "F8"        => KeyCode::F(8),
        "F9"        => KeyCode::F(9),
        "F10"       => KeyCode::F(10),
        s if s.chars().count() == 1 => KeyCode::Char(s.chars().next()?),
        _ => return None,
    })
}
