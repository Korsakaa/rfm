use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, time::Duration};

mod app;
mod config;
mod panel;
mod ui;

fn main() -> io::Result<()> {
    let keys    = config::load_keymap();
    let openers = config::load_openers();
    let mut app = app::App::new(keys, openers);

    // Инициализация терминала
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend  = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let result = run(&mut term, &mut app);

    // Восстановление терминала при любом исходе
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;

    result
}

fn run(
    term: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app:  &mut app::App,
) -> io::Result<()> {
    while app.running {
        // panels area = total − 1(status) − 2(hotkeys) = total − 3
        // panel inner = panels area − 2(borders) = total − 5
        let height = term.size()?.height.saturating_sub(5) as usize;

        app.tick(); // обрабатываем файлы если идёт копирование/перемещение
        term.draw(|f| ui::render(f, app))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key, height);
                }
            }
        }

        // Если нужен sudo — временно уходим из TUI, запускаем команды
        if let Some(sudo_ops) = app.pending_sudo.take() {
            disable_raw_mode()?;
            execute!(term.backend_mut(), LeaveAlternateScreen)?;

            let mut all_ok = true;
            for (prog, args) in &sudo_ops.commands {
                let status = std::process::Command::new("sudo")
                    .arg(prog)
                    .args(args)
                    .status();
                match status {
                    Ok(s) if s.success() => {}
                    Ok(_)  => { all_ok = false; }
                    Err(e) => { all_ok = false; eprintln!("sudo {prog}: {e}"); }
                }
            }

            enable_raw_mode()?;
            execute!(term.backend_mut(), EnterAlternateScreen)?;
            term.clear()?;

            app.left.reload();
            app.right.reload();
            app.message = Some(if all_ok {
                "Права применены".into()
            } else {
                "Ошибка при применении прав (sudo)".into()
            });
        }
    }
    Ok(())
}
