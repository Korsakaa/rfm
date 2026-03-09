use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph},
    Frame,
};
use crate::app::{App, ChmodState, ConfirmAction, InputState, Mode, ProgressOp, ProgressState, SearchFocus, SearchState, Side, UsbMenuState};
use crate::panel::Panel;

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1), // 状态栏
            Constraint::Length(2), // 快捷键栏
        ])
        .split(area);

    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    let height = panels[0].height.saturating_sub(2) as usize;

    render_panel(f, &app.left,  panels[0], matches!(app.active, Side::Left),  height);
    render_panel(f, &app.right, panels[1], matches!(app.active, Side::Right), height);
    render_status(f, app, rows[1]);
    render_hotkeys(f, rows[2]);

    // 覆盖层
    match &app.mode {
        Mode::Confirm(act)    => render_confirm(f, act, area),
        Mode::Input(state)    => render_input(f, state, area),
        Mode::Progress(state) => render_progress(f, state, area),
        Mode::Chmod(state)    => render_chmod(f, state, area),
        Mode::UsbMenu(state)  => render_usb_menu(f, state, area),
        Mode::Search(state)   => {
            let search_area = match app.active {
                Side::Left  => panels[1],
                Side::Right => panels[0],
            };
            render_search(f, state, search_area);
        }
        Mode::Normal => {}
    }
}

// ─── 文件面板 ─────────────────────────────────────────────────────────────────

fn render_panel(f: &mut Frame, panel: &Panel, area: Rect, active: bool, _height: usize) {
    let border_style = if active {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let path = panel.path.to_string_lossy();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            format!(" {} ", path),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let items: Vec<ListItem> = panel.entries
        .iter()
        .enumerate()
        .skip(panel.scroll)
        .take(inner.height as usize)
        .map(|(i, entry)| {
            let cursor   = i == panel.cursor;
            let selected = panel.selected.contains(&i);

            let style = if cursor && active {
                Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)
            } else if cursor {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else if selected {
                // 已选文件 — 黄色
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else if entry.is_dir {
                // 目录 — 蓝色
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
            } else {
                // 文件 — 绿色
                Style::default().fg(Color::Green)
            };

            let mark = if selected { "*" } else { " " };
            let name = if entry.is_dir && entry.name != ".." {
                format!("{mark} {}/", entry.name)
            } else {
                format!("{mark} {}", entry.name)
            };

            ListItem::new(Line::from(vec![Span::styled(name, style)]))
        })
        .collect();

    f.render_widget(List::new(items), inner);
}

// ─── 状态栏 ───────────────────────────────────────────────────────────────────

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let text = if let Some(msg) = &app.message {
        msg.clone()
    } else {
        let p = app.active_panel();
        let total = p.entries.len().saturating_sub(1);
        let sel   = p.selected.len();
        match p.current_entry() {
            Some(e) if e.is_dir && e.name != ".." =>
                format!("  [目录] {}  |  项目：{}  已选：{}", e.name, total, sel),
            Some(e) if e.name != ".." =>
                format!("  {}  {}  |  项目：{}  已选：{}", e.name, fmt_size(e.size), total, sel),
            _ =>
                format!("  项目：{}  已选：{}", total, sel),
        }
    };

    f.render_widget(
        Paragraph::new(text)
            .style(Style::default().bg(Color::DarkGray).fg(Color::White)),
        area,
    );
}

fn fmt_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}K", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}M", bytes as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.1}G", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    }
}

// ─── 快捷键栏 ─────────────────────────────────────────────────────────────────

fn render_hotkeys(f: &mut Frame, area: Rect) {
    let line1 = " Tab/[:左面板   ]:右面板   ↑↓:导航   Alt+↑↓:跳转   空格:选择   Alt+u:USB驱动器";
    let line2 = " F2:新建文件   F3:搜索   F5:复制   F6:移动   F7:新建文件夹   F8:删除   Alt+f:权限   Alt+↵:打开方式   \\ :根目录   r:刷新   q:退出";
    let text = format!("{}\n{}", line1, line2);
    f.render_widget(
        Paragraph::new(text).style(Style::default().bg(Color::Blue).fg(Color::White)),
        area,
    );
}

// ─── 确认对话框 ───────────────────────────────────────────────────────────────

fn render_confirm(f: &mut Frame, action: &ConfirmAction, area: Rect) {
    let popup = centered(60, 5, area);
    f.render_widget(Clear, popup);

    let msg = match action {
        ConfirmAction::Delete(paths) =>
            format!("  删除 {}  个项目？    [y] 是    [n/Esc] 否", paths.len()),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(Span::styled(" 确认 ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)));

    let inner = block.inner(popup);
    f.render_widget(block, popup);
    f.render_widget(
        Paragraph::new(msg).style(Style::default().fg(Color::White)),
        inner,
    );
}

// ─── 输入对话框 ───────────────────────────────────────────────────────────────

fn render_input(f: &mut Frame, state: &InputState, area: Rect) {
    let popup = centered(60, 5, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(" 输入 ", Style::default().fg(Color::Cyan)));

    let inner = block.inner(popup);
    f.render_widget(block, popup);
    f.render_widget(
        Paragraph::new(format!("  {}{}▌", state.prompt, state.value))
            .style(Style::default().fg(Color::White)),
        inner,
    );
}

// ─── 复制/移动进度 ────────────────────────────────────────────────────────────

fn render_progress(f: &mut Frame, state: &ProgressState, area: Rect) {
    let popup = centered(60, 7, area);
    f.render_widget(Clear, popup);

    let op_name = match state.op {
        ProgressOp::Copy => "正在复制",
        ProgressOp::Move => "正在移动",
    };
    let total = state.files.len();
    let done  = state.done;
    let pct   = if total > 0 { (done * 100 / total) as u16 } else { 100 };

    let current_name = state.files.get(done.saturating_sub(1))
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(Span::styled(
            format!(" {} ", op_name),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(format!("  文件 {} / {}", done, total))
            .style(Style::default().fg(Color::White)),
        rows[0],
    );

    let max_w = rows[1].width.saturating_sub(4) as usize;
    let name_display = if current_name.len() > max_w {
        format!("  …{}", &current_name[current_name.len().saturating_sub(max_w)..])
    } else {
        format!("  {}", current_name)
    };
    f.render_widget(
        Paragraph::new(name_display).style(Style::default().fg(Color::Cyan)),
        rows[1],
    );

    f.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
            .percent(pct)
            .label(format!("{}%", pct)),
        rows[2],
    );
}

// ─── 权限对话框 ───────────────────────────────────────────────────────────────

fn render_chmod(f: &mut Frame, state: &ChmodState, area: Rect) {
    let popup = centered(64, 13, area);
    f.render_widget(Clear, popup);

    let max_name = popup.width.saturating_sub(12) as usize;
    let title_name = if state.name.chars().count() > max_name {
        format!("…{}", state.name.chars().rev().take(max_name).collect::<String>().chars().rev().collect::<String>())
    } else {
        state.name.clone()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .title(Span::styled(
            format!(" 权限：{} ", title_name),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // 0: 列标题
            Constraint::Length(1), // 1: 分隔线
            Constraint::Length(1), // 2: 所有者行
            Constraint::Length(1), // 3: 组行
            Constraint::Length(1), // 4: 其他行
            Constraint::Length(1), // 5: 空行
            Constraint::Length(1), // 6: 所有者信息 / sudo 提示
            Constraint::Length(1), // 7: 更改所有者复选框
            Constraint::Length(1), // 8: 递归复选框
            Constraint::Length(1), // 9: 空行
            Constraint::Length(1), // 10: 提示
        ])
        .split(inner);

    let pad_l = "    ";
    let pad_r = "     ";

    let header = format!(
        "{:14}{:^12}{:^12}{:^12}",
        "", "读", "写", "执行"
    );
    f.render_widget(
        Paragraph::new(header).style(Style::default().fg(Color::DarkGray)),
        rows[0],
    );
    f.render_widget(
        Paragraph::new(format!("{}{}", " ".repeat(14), "─".repeat(36)))
            .style(Style::default().fg(Color::DarkGray)),
        rows[1],
    );

    let label_names = ["所有者", "组", "其他"];
    for (row_idx, label_name) in label_names.iter().enumerate() {
        let base = row_idx * 3;
        let label = format!("  {:<12}", label_name);
        let mut spans: Vec<Span> = vec![
            Span::styled(label, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ];
        for col_idx in 0..3 {
            let perm_idx = base + col_idx;
            let active  = state.cursor == perm_idx;
            let checked = state.perms[perm_idx];
            let cb = if checked { "[x]" } else { "[ ]" };

            spans.push(Span::raw(pad_l));
            let style = if active {
                Style::default().fg(Color::Black).bg(Color::Magenta).add_modifier(Modifier::BOLD)
            } else if checked {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            spans.push(Span::styled(cb, style));
            spans.push(Span::raw(pad_r));
        }
        f.render_widget(Paragraph::new(Line::from(spans)), rows[2 + row_idx]);
    }

    // 所有者信息 / sudo 提示
    let owner_line = if state.needs_sudo {
        format!("  ! 所有者：{}（需要 sudo）", state.owner_name)
    } else {
        format!("  所有者：{}", state.owner_name)
    };
    let owner_style = if state.needs_sudo {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(Paragraph::new(owner_line).style(owner_style), rows[6]);

    // 复选框：将所有权转移给当前用户
    {
        let active  = state.cursor == 9;
        let checked = state.change_owner;
        let cb      = if checked { "[x]" } else { "[ ]" };
        let cb_style = if active {
            Style::default().fg(Color::Black).bg(Color::Magenta).add_modifier(Modifier::BOLD)
        } else if checked {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let spans = vec![
            Span::styled("  更改所有者为 ", Style::default().fg(Color::White)),
            Span::styled(state.current_user.as_str(), Style::default().fg(Color::Cyan)),
            Span::raw("  "),
            Span::styled(cb, cb_style),
        ];
        f.render_widget(Paragraph::new(Line::from(spans)), rows[7]);
    }

    // 复选框：递归应用（仅目录）
    {
        let active  = state.cursor == 10;
        let checked = state.recursive;
        let cb      = if checked { "[x]" } else { "[ ]" };
        let cb_style = if active {
            Style::default().fg(Color::Black).bg(Color::Magenta).add_modifier(Modifier::BOLD)
        } else if checked {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        if state.is_dir {
            let spans = vec![
                Span::styled("  递归应用  ", Style::default().fg(Color::White)),
                Span::styled(cb, cb_style),
            ];
            f.render_widget(Paragraph::new(Line::from(spans)), rows[8]);
        }
    }

    // 提示
    f.render_widget(
        Paragraph::new("  ←↑↓→/hjkl：导航   空格：切换   Enter：应用   Esc：取消")
            .style(Style::default().fg(Color::DarkGray)),
        rows[10],
    );
}

// ─── 搜索 ─────────────────────────────────────────────────────────────────────

fn render_search(f: &mut Frame, state: &SearchState, panel_area: Rect) {
    let h = 7u16.min(panel_area.height);
    let popup = Rect::new(panel_area.x, panel_area.y, panel_area.width, h);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(Span::styled(
            " 搜索 (F3) ",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // 输入框
            Constraint::Length(1), // 分隔线
            Constraint::Length(1), // 按钮
            Constraint::Min(0),    // 提示
        ])
        .split(inner);

    // 输入框
    let input_active = state.focus == SearchFocus::Input;
    let cursor = if input_active { "█" } else { " " };
    let input_text = format!(" ▶ {}{}", state.query, cursor);
    let input_style = if input_active {
        Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    f.render_widget(Paragraph::new(input_text).style(input_style), rows[0]);

    // 分隔线
    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize))
            .style(Style::default().fg(Color::DarkGray)),
        rows[1],
    );

    // 按钮
    let btn = |label: &str, active: bool| -> Span {
        if active {
            Span::styled(
                format!(" {} ", label),
                Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                format!(" {} ", label),
                Style::default().fg(Color::White).bg(Color::DarkGray),
            )
        }
    };

    let not_found = state.last_idx.is_none() && !state.query.is_empty();
    let buttons = Line::from(vec![
        Span::raw(" "),
        btn("◄ 上一个", state.focus == SearchFocus::BtnPrev),
        Span::raw("  "),
        btn("下一个 ►", state.focus == SearchFocus::BtnNext),
        Span::raw("  "),
        btn("✕ 关闭",   state.focus == SearchFocus::BtnClose),
        Span::raw(if not_found { "  未找到" } else { "" }),
    ]);
    f.render_widget(Paragraph::new(buttons), rows[2]);

    // 提示
    if rows[3].height > 0 {
        f.render_widget(
            Paragraph::new("  Tab：按钮   Enter：搜索   Esc：关闭")
                .style(Style::default().fg(Color::DarkGray)),
            rows[3],
        );
    }
}

// ─── USB 菜单 ─────────────────────────────────────────────────────────────────

fn render_usb_menu(f: &mut Frame, state: &UsbMenuState, area: Rect) {
    let inner_rows = (state.entries.len() as u16).max(1) + 3;
    let popup_h    = inner_rows.min(20) + 2;
    let popup      = centered(78, popup_h, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " USB 驱动器（Alt+u：刷新）",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    if state.entries.is_empty() {
        f.render_widget(
            Paragraph::new("  未找到 USB 驱动器或未挂载")
                .style(Style::default().fg(Color::DarkGray)),
            inner,
        );
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // 列标题
            Constraint::Length(1), // 分隔线
            Constraint::Min(0),    // 列表
            Constraint::Length(1), // 提示
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(format!(
            "  {:<10} {:<8} {:<22} {}",
            "设备", "大小", "标签", "挂载点"
        ))
        .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        rows[0],
    );
    f.render_widget(
        Paragraph::new("  ".to_string() + &"─".repeat(inner.width.saturating_sub(2) as usize))
            .style(Style::default().fg(Color::DarkGray)),
        rows[1],
    );

    let items: Vec<ListItem> = state.entries.iter().enumerate().map(|(i, e)| {
        let label = if e.label.is_empty() { "—".to_string() } else { e.label.clone() };
        let text  = format!("  {:<10} {:<8} {:<22} {}", e.name, e.size, label, e.mountpoint);
        let style = if i == state.cursor {
            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        ListItem::new(Span::styled(text, style))
    }).collect();

    f.render_widget(List::new(items), rows[2]);

    f.render_widget(
        Paragraph::new("  ↑↓：选择   Enter：导航   Alt+u：刷新   Esc：关闭")
            .style(Style::default().fg(Color::DarkGray)),
        rows[3],
    );
}

// ─── 工具函数 ─────────────────────────────────────────────────────────────────

fn centered(pct_w: u16, lines_h: u16, r: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(lines_h),
            Constraint::Fill(1),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_w) / 2),
            Constraint::Percentage(pct_w),
            Constraint::Percentage((100 - pct_w) / 2),
        ])
        .split(vert[1])[1]
}
