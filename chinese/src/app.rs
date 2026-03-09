use std::{collections::HashMap, mem, path::PathBuf, time::Instant};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::{config::{parse_key, Keys}, panel::Panel};

pub enum Side { Left, Right }

pub enum Mode {
    Normal,
    Confirm(ConfirmAction),
    Input(InputState),
    Progress(ProgressState),
    Chmod(ChmodState),
    UsbMenu(UsbMenuState),
    Search(SearchState),
}

// ─── Search ───────────────────────────────────────────────────────────────────

#[derive(PartialEq)]
pub enum SearchFocus { Input, BtnPrev, BtnNext, BtnClose }

pub struct SearchState {
    pub query:    String,
    pub focus:    SearchFocus,
    pub last_idx: Option<usize>,
}

// ─── USB menu ─────────────────────────────────────────────────────────────────

pub struct UsbEntry {
    pub name:       String,
    pub size:       String,
    pub label:      String,
    pub mountpoint: String,
}

pub struct UsbMenuState {
    pub entries: Vec<UsbEntry>,
    pub cursor:  usize,
}

pub struct ChmodState {
    pub path:         PathBuf,
    pub name:         String,
    pub perms:        [bool; 9], // owner_r/w/x, group_r/w/x, other_r/w/x
    pub cursor:       usize,     // 0-8: perms, 9: change_owner, 10: recursive
    pub is_dir:       bool,
    pub needs_sudo:   bool,      // current user is not the owner
    pub change_owner: bool,      // transfer ownership to current user
    pub recursive:    bool,      // apply recursively
    pub owner_name:   String,    // name of the file owner
    pub current_user: String,    // name of the current user
}

impl ChmodState {
    pub fn from_path(path: PathBuf) -> Option<Self> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let meta   = std::fs::metadata(&path).ok()?;
        let mode   = meta.permissions().mode();
        let is_dir = meta.is_dir();
        let name   = path.file_name()?.to_string_lossy().into_owned();

        let file_uid    = meta.uid();
        let current_uid = get_uid();
        let needs_sudo  = file_uid != current_uid && current_uid != 0;

        let owner_name   = uid_to_name(file_uid).unwrap_or_else(|| file_uid.to_string());
        let current_user = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_default();

        Some(Self {
            perms: [
                mode & 0o400 != 0, mode & 0o200 != 0, mode & 0o100 != 0,
                mode & 0o040 != 0, mode & 0o020 != 0, mode & 0o010 != 0,
                mode & 0o004 != 0, mode & 0o002 != 0, mode & 0o001 != 0,
            ],
            cursor: 0,
            name,
            path,
            is_dir,
            needs_sudo,
            change_owner: false,
            recursive:    false,
            owner_name,
            current_user,
        })
    }

    pub fn to_mode(&self) -> u32 {
        let bits = [0o400u32, 0o200, 0o100, 0o040, 0o020, 0o010, 0o004, 0o002, 0o001];
        bits.iter().zip(self.perms.iter())
            .filter(|(_, &set)| set)
            .map(|(&bit, _)| bit)
            .fold(0, |acc, bit| acc | bit)
    }

}

/// Get UID of the current process
fn get_uid() -> u32 {
    std::process::Command::new("id").arg("-u").output().ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(u32::MAX)
}

/// Get username by UID
fn uid_to_name(uid: u32) -> Option<String> {
    std::process::Command::new("getent")
        .args(["passwd", &uid.to_string()])
        .output().ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.split(':').next().map(|n| n.to_string()))
}

/// Commands to execute via sudo
pub struct SudoOps {
    pub commands: Vec<(String, Vec<String>)>, // (program, arguments)
}

pub struct ProgressState {
    pub files:  Vec<PathBuf>,
    pub dst:    PathBuf,
    pub done:   usize,
    pub errors: usize,
    pub op:     ProgressOp,
}

pub enum ProgressOp { Copy, Move }

pub enum ConfirmAction {
    Delete(Vec<PathBuf>),
}

pub struct InputState {
    pub prompt: String,
    pub value:  String,
    pub action: InputAction,
}

pub enum InputAction {
    Mkdir,
    CreateFile,
    SetOpener { ext: String },
}

pub struct App {
    pub left:         Panel,
    pub right:        Panel,
    pub active:       Side,
    pub keys:         Keys,
    pub openers:      HashMap<String, String>,
    pub message:      Option<String>,
    pub running:      bool,
    pub mode:         Mode,
    pub pending_sudo: Option<SudoOps>,
}

impl App {
    pub fn new(keys: Keys, openers: HashMap<String, String>) -> Self {
        let cwd  = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            left:    Panel::new(cwd),
            right:   Panel::new(home),
            active:  Side::Left,
            keys,
            openers,
            message:      None,
            running:      true,
            mode:         Mode::Normal,
            pending_sudo: None,
        }
    }

    pub fn active_panel(&self) -> &Panel {
        match self.active { Side::Left => &self.left, Side::Right => &self.right }
    }

    pub fn active_panel_mut(&mut self) -> &mut Panel {
        match self.active { Side::Left => &mut self.left, Side::Right => &mut self.right }
    }

    pub fn inactive_panel(&self) -> &Panel {
        match self.active { Side::Left => &self.right, Side::Right => &self.left }
    }

    /// Called every frame. Processes files for up to ~30ms then yields for redraw.
    pub fn tick(&mut self) {
        let Mode::Progress(ref mut state) = self.mode else { return };

        let start = Instant::now();
        while state.done < state.files.len() {
            let src = state.files[state.done].clone();
            let dst = state.dst.join(src.file_name().unwrap_or_default());
            let ok = match state.op {
                ProgressOp::Copy => {
                    if src.is_dir() { copy_dir(&src, &dst).is_ok() }
                    else            { std::fs::copy(&src, &dst).is_ok() }
                }
                ProgressOp::Move => std::fs::rename(&src, &dst).is_ok(),
            };
            if !ok { state.errors += 1; }
            state.done += 1;

            // Yield every ~30ms to redraw the progress bar
            if start.elapsed().as_millis() >= 30 { break; }
        }

        if state.done >= state.files.len() {
            let op    = match state.op { ProgressOp::Copy => "已复制", ProgressOp::Move => "已移动" };
            let done  = state.done - state.errors;
            let err   = state.errors;
            let total = state.files.len();
            self.left.selected.clear();
            self.right.selected.clear();
            self.left.reload();
            self.right.reload();
            self.message = Some(if err == 0 {
                format!("{op}：{done}/{total}")
            } else {
                format!("{op}：{done}/{total}（错误：{err}）")
            });
            self.mode = Mode::Normal;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, height: usize) {
        self.message = None;
        match mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Normal           => self.handle_normal(key, height),
            Mode::Confirm(act)     => self.handle_confirm(key, act),
            Mode::Input(state)     => self.handle_input(key, state, height),
            Mode::Progress(s)      => { self.mode = Mode::Progress(s); }
            Mode::Chmod(state)     => self.handle_chmod(key, state),
            Mode::UsbMenu(state)   => self.handle_usb_menu(key, state),
            Mode::Search(state)    => self.handle_search(key, state, height),
        }
    }

    fn matches(&self, key: KeyEvent, binding: &str) -> bool {
        parse_key(binding).map(|k| k == key).unwrap_or(false)
    }

    fn handle_normal(&mut self, key: KeyEvent, height: usize) {
        let k = self.keys.clone();
        let jump = k.jump_amount as isize;

        if self.matches(key, &k.quit) {
            self.running = false;

        } else if self.matches(key, &k.usb_menu)
            || key == KeyEvent::new(KeyCode::Char('u'), KeyModifiers::ALT)
        {
            let entries = list_usb_mounts();
            self.mode = Mode::UsbMenu(UsbMenuState { entries, cursor: 0 });

        } else if self.matches(key, &k.panel_switch)
            || key == KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)
        {
            self.active = match self.active {
                Side::Left => Side::Right,
                Side::Right => Side::Left,
            };

        } else if self.matches(key, &k.panel_left) {
            self.active = Side::Left;

        } else if self.matches(key, &k.panel_right) {
            self.active = Side::Right;

        } else if self.matches(key, &k.search)
            || key == KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE)
        {
            self.mode = Mode::Search(SearchState {
                query:    String::new(),
                focus:    SearchFocus::Input,
                last_idx: None,
            });

        } else if self.matches(key, &k.chmod) {
            let path = self.active_panel().current_entry()
                .filter(|e| e.name != "..")
                .map(|e| self.active_panel().path.join(&e.name));
            if let Some(path) = path {
                if let Some(state) = ChmodState::from_path(path) {
                    self.mode = Mode::Chmod(state);
                }
            }

        // ── Navigation ───────────────────────────────────────────────────────
        } else if key == KeyEvent::new(KeyCode::Down, KeyModifiers::NONE) {
            self.active_panel_mut().move_cursor(1, height);

        } else if key == KeyEvent::new(KeyCode::Up, KeyModifiers::NONE) {
            self.active_panel_mut().move_cursor(-1, height);

        } else if self.matches(key, &k.jump_down) {
            self.active_panel_mut().move_cursor(jump, height);

        } else if self.matches(key, &k.jump_up) {
            self.active_panel_mut().move_cursor(-jump, height);

        } else if self.matches(key, &k.page_down)
            || key == KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)
        {
            self.active_panel_mut().move_cursor(height as isize, height);

        } else if self.matches(key, &k.page_up)
            || key == KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)
        {
            self.active_panel_mut().move_cursor(-(height as isize), height);

        } else if self.matches(key, &k.go_top) {
            self.active_panel_mut().cursor_to(0, height);

        } else if self.matches(key, &k.go_bottom) {
            let last = self.active_panel().entries.len().saturating_sub(1);
            self.active_panel_mut().cursor_to(last, height);

        // ── Enter / Leave directory ───────────────────────────────────────────
        } else if self.matches(key, &k.enter)
            || key == KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            || key == KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)
        {
            let entry = self.active_panel().current_entry().cloned();
            if let Some(e) = entry {
                if e.is_dir {
                    self.active_panel_mut().enter();
                } else {
                    self.open_file(&e.name.clone());
                }
            }

        } else if self.matches(key, &k.go_up)
            || key == KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)
            || key == KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)
        {
            self.active_panel_mut().go_up();

        // ── Selection ────────────────────────────────────────────────────────
        } else if self.matches(key, &k.select)
            || key == KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)
        {
            self.active_panel_mut().toggle_select();
            self.active_panel_mut().move_cursor(1, height);

        // ── File operations ───────────────────────────────────────────────────
        } else if self.matches(key, &k.copy)
            || key == KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE)
        {
            self.do_copy();

        } else if self.matches(key, &k.move_files)
            || key == KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE)
        {
            self.do_move();

        } else if self.matches(key, &k.create_file)
            || key == KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE)
        {
            self.mode = Mode::Input(InputState {
                prompt: "新建文件：".into(),
                value:  String::new(),
                action: InputAction::CreateFile,
            });

        } else if self.matches(key, &k.mkdir)
            || key == KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE)
        {
            self.mode = Mode::Input(InputState {
                prompt: "新建文件夹：".into(),
                value:  String::new(),
                action: InputAction::Mkdir,
            });

        } else if self.matches(key, &k.delete)
            || key == KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE)
        {
            let paths = self.active_panel().selected_paths();
            if paths.is_empty() {
                self.message = Some("未选择文件".into());
            } else {
                self.mode = Mode::Confirm(ConfirmAction::Delete(paths));
            }

        } else if key == KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT) {
            // Set / change the opener program for the current file extension
            let entry = self.active_panel().current_entry().cloned();
            if let Some(e) = entry {
                if !e.is_dir && e.name != ".." {
                    let ext = std::path::Path::new(&e.name)
                        .extension()
                        .and_then(|x| x.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let current = self.openers.get(&ext).cloned().unwrap_or_default();
                    self.mode = Mode::Input(InputState {
                        prompt: format!("用什么打开 .{ext}："),
                        value:  current,
                        action: InputAction::SetOpener { ext },
                    });
                }
            }

        } else if self.matches(key, &k.refresh) {
            self.left.reload();
            self.right.reload();
            self.message = Some("已刷新".into());

        } else if self.matches(key, &k.go_root) {
            self.active_panel_mut().path = PathBuf::from("/");
            self.active_panel_mut().cursor = 0;
            self.active_panel_mut().scroll = 0;
            self.active_panel_mut().selected.clear();
            self.active_panel_mut().reload();
        }
    }

    fn handle_confirm(&mut self, key: KeyEvent, action: ConfirmAction) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                match action {
                    ConfirmAction::Delete(paths) => {
                        let n = paths.len();
                        for path in &paths {
                            if path.is_dir() {
                                let _ = std::fs::remove_dir_all(path);
                            } else {
                                let _ = std::fs::remove_file(path);
                            }
                        }
                        self.left.selected.clear();
                        self.right.selected.clear();
                        self.left.reload();
                        self.right.reload();
                        self.message = Some(format!("已删除：{n} 个项目"));
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => {}
            _ => { self.mode = Mode::Confirm(action); }
        }
    }

    fn handle_input(&mut self, key: KeyEvent, mut state: InputState, height: usize) {
        match key.code {
            KeyCode::Enter => {
                match state.action {
                    InputAction::CreateFile => {
                        if !state.value.is_empty() {
                            let path = self.active_panel().path.join(&state.value);
                            match std::fs::File::create(&path) {
                                Ok(_) => {
                                    let name = state.value.clone();
                                    self.active_panel_mut().reload();
                                    if let Some(pos) = self.active_panel().entries.iter().position(|e| e.name == name) {
                                        self.active_panel_mut().cursor_to(pos, height);
                                    }
                                    self.message = Some(format!("已创建：{name}"));
                                }
                                Err(e) => self.message = Some(format!("错误：{e}")),
                            }
                        }
                    }
                    InputAction::Mkdir => {
                        if !state.value.is_empty() {
                            let path = self.active_panel().path.join(&state.value);
                            match std::fs::create_dir_all(&path) {
                                Ok(_)  => self.message = Some(format!("已创建：{}", state.value)),
                                Err(e) => self.message = Some(format!("错误：{e}")),
                            }
                            self.active_panel_mut().reload();
                        }
                    }
                    InputAction::SetOpener { ext } => {
                        let prog = state.value.trim().to_string();
                        if !prog.is_empty() {
                            self.openers.insert(ext.clone(), prog.clone());
                            save_opener(&ext, &prog);
                            self.message = Some(format!(".{ext} → {prog}"));
                        }
                    }
                }
            }
            KeyCode::Esc => {}
            KeyCode::Backspace => {
                state.value.pop();
                self.mode = Mode::Input(state);
            }
            KeyCode::Char(c) => {
                state.value.push(c);
                self.mode = Mode::Input(state);
            }
            _ => { self.mode = Mode::Input(state); }
        }
    }

    fn open_file(&mut self, name: &str) {
        let ext = PathBuf::from(name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let path = self.active_panel().path.join(name);
        match self.openers.get(&ext).cloned() {
            Some(prog) => {
                if let Err(e) = std::process::Command::new(&prog)
                    .arg(&path)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    self.message = Some(format!("启动 {prog} 失败：{e}"));
                }
            }
            None => {
                self.message = Some(
                    format!("无法打开 .{ext}（请添加到 ~/.config/rfm/openers.toml）")
                );
            }
        }
    }

    fn handle_search(&mut self, key: KeyEvent, mut state: SearchState, height: usize) {
        match key.code {
            KeyCode::Esc => {}  // close — mode stays Normal

            // Tab / BackTab — cycle focus
            KeyCode::Tab => {
                state.focus = match state.focus {
                    SearchFocus::Input    => SearchFocus::BtnNext,
                    SearchFocus::BtnNext  => SearchFocus::BtnPrev,
                    SearchFocus::BtnPrev  => SearchFocus::BtnClose,
                    SearchFocus::BtnClose => SearchFocus::Input,
                };
                self.mode = Mode::Search(state);
            }
            KeyCode::BackTab => {
                state.focus = match state.focus {
                    SearchFocus::Input    => SearchFocus::BtnClose,
                    SearchFocus::BtnNext  => SearchFocus::Input,
                    SearchFocus::BtnPrev  => SearchFocus::BtnNext,
                    SearchFocus::BtnClose => SearchFocus::BtnPrev,
                };
                self.mode = Mode::Search(state);
            }

            KeyCode::Enter => {
                match state.focus {
                    SearchFocus::BtnClose => { /* close */ }

                    SearchFocus::Input | SearchFocus::BtnNext => {
                        if !state.query.is_empty() {
                            let from = state.last_idx.unwrap_or_else(|| self.active_panel().cursor);
                            match self.find_match_next(&state.query, from) {
                                Some(pos) => {
                                    state.last_idx = Some(pos);
                                    self.active_panel_mut().cursor_to(pos, height);
                                }
                                None => self.message = Some(format!("未找到「{}」", state.query)),
                            }
                        }
                        self.mode = Mode::Search(state);
                    }

                    SearchFocus::BtnPrev => {
                        if !state.query.is_empty() {
                            let from = state.last_idx.unwrap_or_else(|| self.active_panel().cursor);
                            match self.find_match_prev(&state.query, from) {
                                Some(pos) => {
                                    state.last_idx = Some(pos);
                                    self.active_panel_mut().cursor_to(pos, height);
                                }
                                None => self.message = Some(format!("未找到「{}」", state.query)),
                            }
                        }
                        self.mode = Mode::Search(state);
                    }
                }
            }

            // Characters always go to the input field
            KeyCode::Char(c) => {
                state.focus = SearchFocus::Input;
                state.query.push(c);
                let len = self.active_panel().entries.len();
                if len > 0 {
                    if let Some(pos) = self.find_match_next(&state.query, len - 1) {
                        state.last_idx = Some(pos);
                        self.active_panel_mut().cursor_to(pos, height);
                    } else {
                        state.last_idx = None;
                    }
                }
                self.mode = Mode::Search(state);
            }

            KeyCode::Backspace => {
                state.query.pop();
                state.last_idx = None;
                if !state.query.is_empty() {
                    let len = self.active_panel().entries.len();
                    if len > 0 {
                        if let Some(pos) = self.find_match_next(&state.query, len - 1) {
                            state.last_idx = Some(pos);
                            self.active_panel_mut().cursor_to(pos, height);
                        }
                    }
                }
                self.mode = Mode::Search(state);
            }

            _ => { self.mode = Mode::Search(state); }
        }
    }

    /// Search forward starting from the element after `from` (wrapping)
    fn find_match_next(&self, query: &str, from: usize) -> Option<usize> {
        let entries = &self.active_panel().entries;
        let q = query.to_lowercase();
        let len = entries.len();
        if len == 0 { return None; }
        for i in 1..=len {
            let idx = (from + i) % len;
            if entries[idx].name.to_lowercase().contains(&q) {
                return Some(idx);
            }
        }
        None
    }

    /// Search backward starting from the element before `from` (wrapping)
    fn find_match_prev(&self, query: &str, from: usize) -> Option<usize> {
        let entries = &self.active_panel().entries;
        let q = query.to_lowercase();
        let len = entries.len();
        if len == 0 { return None; }
        for i in 1..=len {
            let idx = (from + len - i) % len;
            if entries[idx].name.to_lowercase().contains(&q) {
                return Some(idx);
            }
        }
        None
    }

    fn handle_usb_menu(&mut self, key: KeyEvent, mut state: UsbMenuState) {
        match key.code {
            KeyCode::Esc => {}  // close — mode stays Normal

            // Refresh list
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::ALT) => {
                state.entries = list_usb_mounts();
                state.cursor  = 0;
                self.mode = Mode::UsbMenu(state);
            }

            KeyCode::Up => {
                if state.cursor > 0 { state.cursor -= 1; }
                self.mode = Mode::UsbMenu(state);
            }
            KeyCode::Down => {
                if state.cursor + 1 < state.entries.len() { state.cursor += 1; }
                self.mode = Mode::UsbMenu(state);
            }

            KeyCode::Enter => {
                if let Some(entry) = state.entries.get(state.cursor) {
                    let path = PathBuf::from(&entry.mountpoint);
                    if path.is_dir() {
                        self.active_panel_mut().path = path;
                        self.active_panel_mut().cursor = 0;
                        self.active_panel_mut().scroll = 0;
                        self.active_panel_mut().selected.clear();
                        self.active_panel_mut().reload();
                        // mode → Normal, 菜单关闭
                    } else {
                        self.message = Some(format!("路径不可用：{}", entry.mountpoint));
                        self.mode = Mode::UsbMenu(state);
                    }
                }
            }

            _ => { self.mode = Mode::UsbMenu(state); }
        }
    }

    fn handle_chmod(&mut self, key: KeyEvent, mut state: ChmodState) {
        match key.code {
            KeyCode::Enter => {
                let mode     = state.to_mode();
                let path_str = state.path.to_string_lossy().into_owned();
                let rec      = state.recursive && state.is_dir;

                if state.needs_sudo || state.change_owner {
                    let mut cmds: Vec<(String, Vec<String>)> = Vec::new();

                    let mut chmod_args = Vec::new();
                    if rec { chmod_args.push("-R".into()); }
                    chmod_args.push(format!("{:o}", mode));
                    chmod_args.push(path_str.clone());
                    cmds.push(("chmod".into(), chmod_args));

                    if state.change_owner && !state.current_user.is_empty() {
                        let mut chown_args = Vec::new();
                        if rec { chown_args.push("-R".into()); }
                        chown_args.push(state.current_user.clone());
                        chown_args.push(path_str);
                        cmds.push(("chown".into(), chown_args));
                    }

                    self.pending_sudo = Some(SudoOps { commands: cmds });
                } else {
                    use std::os::unix::fs::PermissionsExt;
                    if rec {
                        match std::process::Command::new("chmod")
                            .arg("-R").arg(format!("{:o}", mode)).arg(&state.path)
                            .status()
                        {
                            Ok(_)  => self.message = Some(format!("权限 {:o} 已递归应用", mode)),
                            Err(e) => self.message = Some(format!("错误：{e}")),
                        }
                    } else {
                        match std::fs::metadata(&state.path)
                            .and_then(|m| { let mut p = m.permissions(); p.set_mode(mode); std::fs::set_permissions(&state.path, p) })
                        {
                            Ok(_)  => self.message = Some(format!("权限：{:o} → {}", mode, state.name)),
                            Err(e) => self.message = Some(format!("错误：{e}")),
                        }
                    }
                    self.active_panel_mut().reload();
                }
            }
            KeyCode::Esc => {}

            KeyCode::Char(' ') => {
                match state.cursor {
                    0..=8 => state.perms[state.cursor] ^= true,
                    9     => state.change_owner ^= true,
                    10    => state.recursive    ^= true,
                    _     => {}
                }
                self.mode = Mode::Chmod(state);
            }

            KeyCode::Left | KeyCode::Char('h') => {
                if state.cursor < 9 && state.cursor % 3 > 0 { state.cursor -= 1; }
                self.mode = Mode::Chmod(state);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if state.cursor < 9 && state.cursor % 3 < 2 { state.cursor += 1; }
                self.mode = Mode::Chmod(state);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                state.cursor = match state.cursor {
                    0..=8 if state.cursor >= 3 => state.cursor - 3,
                    9  => 6,
                    10 => 9,
                    _  => state.cursor,
                };
                self.mode = Mode::Chmod(state);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                state.cursor = match state.cursor {
                    0..=5 => state.cursor + 3,
                    6..=8 => 9,
                    9 if state.is_dir => 10,
                    _  => state.cursor,
                };
                self.mode = Mode::Chmod(state);
            }

            _ => { self.mode = Mode::Chmod(state); }
        }
    }

    fn do_copy(&mut self) {
        let files = self.active_panel().selected_paths();
        if files.is_empty() {
            self.message = Some("没有可复制的文件".into());
            return;
        }
        let dst = self.inactive_panel().path.clone();
        self.mode = Mode::Progress(ProgressState { files, dst, done: 0, errors: 0, op: ProgressOp::Copy });
    }

    fn do_move(&mut self) {
        let files = self.active_panel().selected_paths();
        if files.is_empty() {
            self.message = Some("没有可移动的文件".into());
            return;
        }
        let dst = self.inactive_panel().path.clone();
        self.mode = Mode::Progress(ProgressState { files, dst, done: 0, errors: 0, op: ProgressOp::Move });
    }
}

// ─── USB drives ───────────────────────────────────────────────────────────────

fn list_usb_mounts() -> Vec<UsbEntry> {
    let output = std::process::Command::new("lsblk")
        .args(["-P", "-o", "NAME,SIZE,TRAN,MOUNTPOINTS,LABEL,HOTPLUG"])
        .output()
        .or_else(|_| {
            std::process::Command::new("lsblk")
                .args(["-P", "-o", "NAME,SIZE,TRAN,MOUNTPOINT,LABEL,HOTPLUG"])
                .output()
        });

    let text = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => return vec![],
    };

    let mut usb_disks: std::collections::HashSet<String> = std::collections::HashSet::new();
    for line in text.lines() {
        let m = parse_lsblk_pairs(line);
        if m.get("TRAN").map(|s| s == "usb").unwrap_or(false) {
            if let Some(name) = m.get("NAME") {
                usb_disks.insert(name.clone());
            }
        }
    }

    let mut entries = Vec::new();
    for line in text.lines() {
        let m = parse_lsblk_pairs(line);

        let name = m.get("NAME").cloned().unwrap_or_default();
        let mountpoint = m.get("MOUNTPOINTS")
            .or_else(|| m.get("MOUNTPOINT"))
            .cloned()
            .unwrap_or_default();

        if mountpoint.is_empty() { continue; }

        let is_usb = m.get("HOTPLUG").map(|s| s == "1").unwrap_or(false)
            || usb_disks.iter().any(|d| name.starts_with(d.as_str()) && name != *d);

        if is_usb {
            entries.push(UsbEntry {
                name,
                size:  m.get("SIZE").cloned().unwrap_or_default(),
                label: m.get("LABEL").cloned().unwrap_or_default(),
                mountpoint,
            });
        }
    }
    entries
}

fn parse_lsblk_pairs(line: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut s = line.trim();
    while !s.is_empty() {
        let Some(eq) = s.find("=\"") else { break };
        let key = s[..eq].trim().to_string();
        s = &s[eq + 2..];
        let Some(close) = s.find('"') else { break };
        let value = s[..close].to_string();
        map.insert(key, value);
        s = s[close + 1..].trim_start();
    }
    map
}

fn save_opener(ext: &str, prog: &str) {
    use crate::config::config_dir;
    let path = config_dir().join("openers.toml");

    let mut root: toml::Table = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();

    let openers = root
        .entry("openers".to_string())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));

    if let toml::Value::Table(t) = openers {
        t.insert(ext.to_string(), toml::Value::String(prog.to_string()));
    }

    let _ = std::fs::write(&path, toml::to_string_pretty(&root).unwrap_or_default());
}

fn copy_dir(src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let d = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &d)?;
        } else {
            std::fs::copy(e.path(), d)?;
        }
    }
    Ok(())
}
