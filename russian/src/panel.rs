use std::{collections::HashSet, fs, path::PathBuf};

#[derive(Debug, Clone)]
pub struct Entry {
    pub name:   String,
    pub is_dir: bool,
    pub size:   u64,
}

pub struct Panel {
    pub path:     PathBuf,
    pub entries:  Vec<Entry>,
    pub cursor:   usize,
    pub scroll:   usize,
    pub selected: HashSet<usize>,
}

impl Panel {
    pub fn new(path: PathBuf) -> Self {
        let mut p = Self {
            path,
            entries:  Vec::new(),
            cursor:   0,
            scroll:   0,
            selected: HashSet::new(),
        };
        p.reload();
        p
    }

    pub fn reload(&mut self) {
        self.entries.clear();
        // Всегда первым — переход вверх
        self.entries.push(Entry { name: "..".into(), is_dir: true, size: 0 });

        if let Ok(read_dir) = fs::read_dir(&self.path) {
            let mut entries: Vec<Entry> = read_dir
                .filter_map(|e| e.ok())
                .map(|e| {
                    let meta = e.metadata().ok();
                    Entry {
                        name:   e.file_name().to_string_lossy().into_owned(),
                        is_dir: meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                        size:   meta.as_ref().map(|m| m.len()).unwrap_or(0),
                    }
                })
                .collect();

            // Сортировка: папки вверху, потом файлы, по алфавиту
            entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            });

            self.entries.extend(entries);
        }

        // Не выходить за пределы после reload
        let max = self.entries.len().saturating_sub(1);
        self.cursor = self.cursor.min(max);
        self.selected.retain(|&i| i < self.entries.len());
    }

    pub fn move_cursor(&mut self, delta: isize, height: usize) {
        let len = self.entries.len();
        if len == 0 { return; }
        let new = (self.cursor as isize + delta).clamp(0, len as isize - 1) as usize;
        self.cursor = new;
        self.fix_scroll(height);
    }

    pub fn cursor_to(&mut self, pos: usize, height: usize) {
        self.cursor = pos.min(self.entries.len().saturating_sub(1));
        self.fix_scroll(height);
    }

    fn fix_scroll(&mut self, height: usize) {
        if height == 0 { return; }
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + height {
            self.scroll = self.cursor + 1 - height;
        }
    }

    pub fn current_entry(&self) -> Option<&Entry> {
        self.entries.get(self.cursor)
    }

    /// Войти в директорию под курсором. Возвращает true если вошли.
    pub fn enter(&mut self) -> bool {
        if let Some(entry) = self.entries.get(self.cursor) {
            if entry.is_dir {
                let new_path = if entry.name == ".." {
                    self.path.parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| self.path.clone())
                } else {
                    self.path.join(&entry.name)
                };
                self.path = new_path;
                self.cursor = 0;
                self.scroll = 0;
                self.selected.clear();
                self.reload();
                return true;
            }
        }
        false
    }

    /// Подняться на уровень вверх, курсор встаёт на папку откуда пришли.
    pub fn go_up(&mut self) {
        if let Some(parent) = self.path.parent().map(|p| p.to_path_buf()) {
            let came_from = self.path.file_name()
                .map(|n| n.to_string_lossy().into_owned());
            self.path = parent;
            self.cursor = 0;
            self.scroll = 0;
            self.selected.clear();
            self.reload();
            if let Some(name) = came_from {
                if let Some(pos) = self.entries.iter().position(|e| e.name == name) {
                    self.cursor = pos;
                }
            }
        }
    }

    pub fn toggle_select(&mut self) {
        let cur = self.cursor;
        if self.entries.get(cur).map(|e| e.name == "..").unwrap_or(true) {
            return; // ".." не выделяем
        }
        if self.selected.contains(&cur) {
            self.selected.remove(&cur);
        } else {
            self.selected.insert(cur);
        }
    }

    /// Возвращает выбранные пути. Если ничего не выбрано — текущий файл.
    pub fn selected_paths(&self) -> Vec<PathBuf> {
        if self.selected.is_empty() {
            self.current_entry()
                .filter(|e| e.name != "..")
                .map(|e| vec![self.path.join(&e.name)])
                .unwrap_or_default()
        } else {
            let mut paths: Vec<PathBuf> = self.selected.iter()
                .filter_map(|&i| self.entries.get(i))
                .filter(|e| e.name != "..")
                .map(|e| self.path.join(&e.name))
                .collect();
            paths.sort();
            paths
        }
    }
}
