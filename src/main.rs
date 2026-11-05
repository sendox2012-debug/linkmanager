use crossterm::cursor::MoveTo;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use dirs::config_dir;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io;
use std::path::PathBuf;
use std::process::Command;

// --- Модели данных ---
#[derive(Serialize, Deserialize, Clone)]
struct Link {
    name: String,
    url: String,
}

#[derive(Clone, PartialEq)]
enum InputMode {
    Normal,
    AddingName,
    AddingUrl,
}

struct App {
    links: Vec<Link>,
    list_state: ListState,
    input_mode: InputMode,
    name_input: String,
    url_input: String,
    config_path: PathBuf,
    should_quit: bool, 
}

impl App {
    fn new() -> Self {
        let mut config_path = config_dir().unwrap_or_else(|| PathBuf::from("."));
        config_path.push("quick_links");
        fs::create_dir_all(&config_path).unwrap_or_default();
        config_path.push("links.json");

        let links = Self::load_links(&config_path);
        let mut list_state = ListState::default();
        if !links.is_empty() {
            list_state.select(Some(0));
        }

        App {
            links,
            list_state,
            input_mode: InputMode::Normal,
            name_input: String::new(),
            url_input: String::new(),
            config_path,
            should_quit: false,
        }
    }

    fn load_links(path: &PathBuf) -> Vec<Link> {
        if path.exists() {
            if let Ok(data) = fs::read_to_string(path) {
                if let Ok(links) = serde_json::from_str(&data) {
                    return links;
                }
            }
        }
        vec![
            Link {
                name: "GitHub".to_string(),
                url: "https://github.com".to_string(),
            },
            Link {
                name: "Rust Book".to_string(),
                url: "https://doc.rust-lang.org/book/".to_string(),
            },
        ]
    }

    fn save_links(&self) {
        if let Ok(mut file) = File::create(&self.config_path) {
            let _ = serde_json::to_writer_pretty(&mut file, &self.links);
        }
    }

    fn next(&mut self) {
        if self.links.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.links.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.links.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.links.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn open_selected(&self) {
        if let Some(i) = self.list_state.selected() {
            if let Some(link) = self.links.get(i) {
                let result = Command::new("firefox").arg(&link.url).spawn();
                if result.is_err() {
                    let _ = webbrowser::open(&link.url);
                }
            }
        }
    }

    fn delete_selected(&mut self) {
        if let Some(i) = self.list_state.selected() {
            self.links.remove(i);
            self.save_links();
            if self.links.is_empty() {
                self.list_state.select(None);
            } else {
                let new_idx = if i >= self.links.len() {
                    self.links.len() - 1
                } else {
                    i
                };
                self.list_state.select(Some(new_idx));
            }
        }
    }

    fn handle_key(&mut self, key: KeyCode) {
        match self.input_mode {
            InputMode::Normal => match key {
                KeyCode::Char('q') => self.should_quit = true, 
                KeyCode::Down | KeyCode::Char('j') => self.next(),
                KeyCode::Up | KeyCode::Char('k') => self.previous(),
                KeyCode::Enter => self.open_selected(),
                KeyCode::Char('a') => self.input_mode = InputMode::AddingName,
                KeyCode::Char('d') => self.delete_selected(),
                _ => {}
            },
            InputMode::AddingName => match key {
                KeyCode::Enter => self.input_mode = InputMode::AddingUrl,
                KeyCode::Esc => {
                    self.name_input.clear();
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Char(c) => self.name_input.push(c),
                KeyCode::Backspace => {
                    self.name_input.pop();
                }
                _ => {}
            },
            InputMode::AddingUrl => match key {
                KeyCode::Enter => {
                    if !self.name_input.is_empty() && !self.url_input.is_empty() {
                        self.links.push(Link {
                            name: self.name_input.clone(),
                            url: self.url_input.clone(),
                        });
                        self.save_links();
                        self.list_state.select(Some(self.links.len() - 1));
                    }
                    self.name_input.clear();
                    self.url_input.clear();
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Esc => {
                    self.name_input.clear();
                    self.url_input.clear();
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Char(c) => self.url_input.push(c),
                KeyCode::Backspace => {
                    self.url_input.pop();
                }
                _ => {}
            },
        }
    }
}

// --- Отрисовка UI ---
fn ui(f: &mut Frame, app: &App) {
    let size = f.area();

    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " Менеджер быстрых ссылок ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ));

    f.render_widget(&main_block, size);

    let inner = main_block.inner(size);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
        .split(inner);

    let items: Vec<ListItem> = app
        .links
        .iter()
        .map(|link| {
            let content = format!(" {}  →  {}", link.name, link.url);
            ListItem::new(content).style(Style::default().fg(Color::White))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::NONE))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, chunks[0], &mut app.list_state.clone());

    let bottom_text = match app.input_mode {
        InputMode::Normal => Span::styled(
            " [Enter] Открыть | [A] Добавить | [D] Удалить | [Q] Выход | [↑/↓] Навигация ",
            Style::default().fg(Color::Gray),
        ),
        InputMode::AddingName => Span::styled(
            format!(" Введите название: {}_", app.name_input),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::ITALIC),
        ),
        InputMode::AddingUrl => Span::styled(
            format!(" Введите ссылку для '{}': {}_", app.name_input, app.url_input),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::ITALIC),
        ),
    };

    let bottom_block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(bottom_text).block(bottom_block);
    f.render_widget(paragraph, chunks[1]);
}

// --- Инициализация терминала ---
fn setup_terminal() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    Ok(())
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        LeaveAlternateScreen,
        DisableMouseCapture,
        Clear(ClearType::All),
        MoveTo(0, 0)
    )?;
    Ok(())
}

fn main() -> io::Result<()> {
    let panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = restore_terminal();
        panic_hook(panic);
    }));

    setup_terminal()?;

    let result = run_app();

    restore_terminal()?;
    result
}

fn run_app() -> io::Result<()> {
    let mut app = App::new();
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                app.handle_key(key.code);
            }
        }

        // Проверяем флаг выхода
        if app.should_quit {
            break;
        }
    }

    Ok(())
}