use std::env;
use std::fs;
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

struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        // Ensure terminal is restored on exit
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
    }
}

fn main() -> Result<(), io::Error> {
    // Setup terminal
    let mut stdout = io::stdout();
    execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    crossterm::terminal::enable_raw_mode()?;
    let _cleanup = TerminalCleanup; // Automatically restores terminal on drop
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut current_dir = PathBuf::from(".");
    let original_dir = current_dir.clone();
    // let mut current_dir = env::current_dir()?;

    let mut list_state = ListState::default();
    list_state.select(Some(0)); // Initialize selection at the first item

    let mut is_viewing_file = false;
    let mut file_content: Vec<String> = Vec::new();
    let mut file_scroll = 0;

    loop {
        if !is_viewing_file {
            // Directory browsing mode
            let entries: Vec<_> = fs::read_dir(&current_dir)?
                .filter_map(Result::ok)
                .collect();

            let items: Vec<ListItem> = entries
                .iter()
                .map(|entry| {
                    let name = entry.file_name().into_string().unwrap_or_default();
                    ListItem::new(Span::styled(
                        name,
                        Style::default().fg(Color::White),
                    ))
                })
                .collect();

            terminal.draw(|f| {
                let size = f.size();
                let layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Percentage(100)].as_ref())
                    .split(size);

                // Display current directory path
                let path_display = Paragraph::new(Spans::from(vec![
                    Span::styled(
                        format!("Current Directory: {:?}", current_dir.display()),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ),
                ]))
                .block(Block::default().borders(Borders::ALL).title("Path"));

                // Display directory list
                let list_block = Block::default().borders(Borders::ALL).title("Directory Browser");
                let list = List::new(items)
                    .block(list_block)
                    .highlight_style(Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD))
                    .highlight_symbol(">> ");

                // Render widgets
                f.render_widget(path_display, layout[0]);
                f.render_stateful_widget(list, layout[1], &mut list_state);
            })?;

            // Handle user input for directory browsing
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                match code {
                    KeyCode::Char('q') => break,
                    KeyCode::Down | KeyCode::Char('s') => {
                        let i = match list_state.selected() {
                            Some(i) => {
                                if i >= entries.len() - 1 {
                                    0
                                } else {
                                    i + 1
                                }
                            }
                            None => 0,
                        };
                        list_state.select(Some(i));
                    }
                    KeyCode::Up | KeyCode::Char('w') => {
                        let i = match list_state.selected() {
                            Some(i) => {
                                if i == 0 {
                                    entries.len() - 1
                                } else {
                                    i - 1
                                }
                            }
                            None => 0,
                        };
                        list_state.select(Some(i));
                    }
                    KeyCode::Enter => {
                        if let Some(i) = list_state.selected() {
                            let path = entries[i].path();
                            if path.is_dir() {
                                current_dir = path;
                                list_state.select(Some(0));
                            } else if path.is_file() {
                                file_content = read_file_lines(&path)?;
                                is_viewing_file = true;
                                file_scroll = 0;
                            }
                        }
                    }
                    KeyCode::Esc  => {
                        if current_dir==original_dir {
                            continue;
                        }
                        if let Some(parent) = current_dir.parent() {
                            current_dir = parent.to_path_buf();
                            list_state.select(Some(0));
                        }
                    }
                    _ => {}
                }
            }
        } else {
            // File viewing mode
            terminal.draw(|f| {
                let size = f.size();
                let block = Block::default().borders(Borders::ALL).title("File Viewer");
                let layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(100)].as_ref())
                    .split(size);

                let content: Vec<Spans> = file_content
                    .iter()
                    .skip(file_scroll)
                    .take(size.height as usize - 2)
                    .map(|line| Spans::from(Span::raw(line.clone())))
                    .collect();
                
                let paragraph = Paragraph::new(content)
                    .block(block);

                f.render_widget(paragraph, layout[0]);
            })?;

            // Handle user input for file viewing
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                match code {
                    KeyCode::Char('q') => break,
                    KeyCode::Up | KeyCode::Char('w') => {
                        if file_scroll > 0 {
                            file_scroll -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('s') => {
                        if file_scroll < file_content.len().saturating_sub(1) {
                            file_scroll += 1;
                        }
                    }
                    KeyCode::Esc => {
                        is_viewing_file = false;
                        list_state.select(Some(0));
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

// Helper function to read file contents into lines
fn read_file_lines(path: &PathBuf) -> io::Result<Vec<String>> {
    let file = File::open(path)?;
    let reader = io::BufReader::new(file);
    Ok(reader.lines().filter_map(Result::ok).collect())
}
