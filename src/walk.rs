use std::fs::File;
use std::io::{self, BufRead};
use std::path::PathBuf;
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::execute;


pub struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
    }
}

pub enum Mode {
    Dir,
    File,
}

pub struct State {
    pub current_dir: PathBuf,
    pub original_dir: PathBuf,
    pub list_state: ListState,
    pub mode: Mode,
    pub file_content: Vec<String>,
    pub file_scroll: usize,
    pub file_path: String,

    pub show_lines: bool
}

impl State {
    pub fn new() -> Self {
        Self {
            current_dir: PathBuf::from("."),
            original_dir: PathBuf::from("."),
            list_state: ListState::default(),
            mode: Mode::Dir,
            file_content: Vec::new(),
            file_scroll: 0,
            file_path: String::new(),

            show_lines: false,
        }
    }
}

pub fn create_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, io::Error> {
    execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
    crossterm::terminal::enable_raw_mode()?;
    let backend = CrosstermBackend::new(io::stdout());
    Terminal::new(backend)
}

pub fn render_directory(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    entries: &[std::fs::DirEntry],
    state: &mut State,
) -> Result<(), Box<dyn std::error::Error>> {
    let items: Vec<ListItem> = entries
        .iter()
        .map(|entry| {
            let name = entry.file_name().into_string().unwrap_or_default();
            ListItem::new(Span::styled(name, Style::default().fg(Color::White)))
        })
        .collect();

    terminal.draw(|f| {
        let size = f.size();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100)].as_ref())
            .split(size);

        let list_block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                format!("Directory Browser - {}", state.current_dir.display()),
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
            ));
        let list = List::new(items)
            .block(list_block)
            .highlight_style(Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        f.render_stateful_widget(list, layout[0], &mut state.list_state);
    })?;
    Ok(())
}

pub fn handle_directory_input(
    entries: &[std::fs::DirEntry],
    state: &mut State,
) -> Result<bool, Box<dyn std::error::Error>> {
    if let Event::Key(KeyEvent { code, .. }) = event::read()? {
        match code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Down | KeyCode::Char('s') => {
                let i = match state.list_state.selected() {
                    Some(i) => (i + 1) % entries.len(),
                    None => 0,
                };
                state.list_state.select(Some(i));
            }
            KeyCode::Up | KeyCode::Char('w') => {
                let i = match state.list_state.selected() {
                    Some(i) => if i == 0 { entries.len() - 1 } else { i - 1 },
                    None => 0,
                };
                state.list_state.select(Some(i));
            }
            KeyCode::Enter => {
                if let Some(i) = state.list_state.selected() {
                    let path = entries[i].path();
                    if path.is_dir() {
                        state.current_dir = path;
                        state.list_state.select(Some(0));
                    } else if path.is_file() {
                        state.file_content = read_file_lines(&path)?;
                        state.file_path = path.display().to_string();
                        state.mode = Mode::File;
                        state.file_scroll = 0;
                    }
                }
            }
            KeyCode::Esc => {
                if state.current_dir != state.original_dir {
                    if let Some(parent) = state.current_dir.parent() {
                        state.current_dir = parent.to_path_buf();
                        state.list_state.select(Some(0));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(false)
}

pub fn render_file_viewer(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &State,
) -> Result<(), Box<dyn std::error::Error>> {
    terminal.draw(|f| {
        let size = f.size();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100)].as_ref())
            .split(size);

        let file_block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                format!("File Viewer - {}", state.file_path),
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
            ));

        let content: Vec<Spans> = state
            .file_content
            .iter()
            .skip(state.file_scroll)
            .take(size.height as usize - 2)
            .enumerate()
            .map(|(i, line)| {
                if state.show_lines {
                    create_line_with_number(i + state.file_scroll, line)
                } else {
                    create_line_without_number(line)
                }
            })
            .collect();

        let paragraph = Paragraph::new(content).block(file_block);
        f.render_widget(paragraph, layout[0]);
    })?;
    Ok(())
}

// Helper function to create a line with a line number
fn create_line_with_number(line_number: usize, line: &str) -> Spans {
    let line_number_span = Span::styled(
        format!("{:<4} ", line_number + 1),
        Style::default().fg(Color::Blue),
    );
    let line_content_span = Span::raw(line.to_string());
    Spans::from(vec![line_number_span, line_content_span])
}

// Helper function to create a line without a line number
fn create_line_without_number(line: &str) -> Spans {
    Spans::from(Span::raw(line.to_string()))
}




pub fn handle_file_input(state: &mut State) -> Result<bool, io::Error> {
    if let Event::Key(KeyEvent { code, .. }) = event::read()? {
        match code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Up | KeyCode::Char('w') => {
                if state.file_scroll > 0 {
                    state.file_scroll -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('s') => {
                if state.file_scroll < state.file_content.len().saturating_sub(1) {
                    state.file_scroll += 1;
                }
            }

            KeyCode::Char('l') => state.show_lines = !state.show_lines, 

            KeyCode::Esc => state.mode = Mode::Dir,
            _ => {}
        }
    }
    Ok(false)
}

pub fn read_file_lines(path: &PathBuf) -> io::Result<Vec<String>> {
    let file = File::open(path)?;
    let reader = io::BufReader::new(file);
    Ok(reader.lines().filter_map(Result::ok).collect())
}
