use std::sync::Arc;
use std::path::Path;
use crate::program_context::CodeFile;
use crate::file_parser::InstructionDetail;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::PathBuf;
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState},
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
    pub dir_list_state: ListState,
    pub mode: Mode,
    pub file_content: Vec<Line>,
    
    pub file_scroll: usize,
    pub cursor: usize,
    pub file_path: String,

    pub show_lines: bool,

    pub asm_cursor: usize,

}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    pub fn new() -> Self {
        Self {
            current_dir: PathBuf::from("."),
            original_dir: PathBuf::from("."),
            dir_list_state: ListState::default(),
            mode: Mode::Dir,
            file_content: Vec::new(),
            file_scroll: 0,
            cursor: 0,
            file_path: String::new(),

            show_lines: false,

            asm_cursor:0,
        }
    }
}

pub struct Line {
    content: String,
    is_selected: bool,
    line_number: usize, // Optionally store the line number
    // debug_info: Option<String>,  // Placeholder for future debug information
}

impl Line {
    fn new(content: String, line_number: usize) -> Self {
        Self {
            content,
            is_selected: false,
            line_number,
            // debug_info: None,
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

        f.render_stateful_widget(list, layout[0], &mut state.dir_list_state);
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
                let i = match state.dir_list_state.selected() {
                    Some(i) => (i + 1) % entries.len(),
                    None => 0,
                };
                state.dir_list_state.select(Some(i));
            }
            KeyCode::Up | KeyCode::Char('w') => {
                let i = match state.dir_list_state.selected() {
                    Some(i) => if i == 0 { entries.len() - 1 } else { i - 1 },
                    None => 0,
                };
                state.dir_list_state.select(Some(i));
            }
            KeyCode::Enter => {
                if let Some(i) = state.dir_list_state.selected() {
                    let path = entries[i].path();
                    if path.is_dir() {
                        state.current_dir = path;
                        state.dir_list_state.select(Some(0));
                    } else if path.is_file() {
                        load_file(state,&path)?;

                    }
                }
            }
            KeyCode::Esc => {
                if state.current_dir != state.original_dir {
                    if let Some(parent) = state.current_dir.parent() {
                        state.current_dir = parent.to_path_buf();
                        state.dir_list_state.select(Some(0));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(false)
}

pub fn load_file(state:&mut State,path:&PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    state.file_content = read_file_lines(path)?;
    state.file_path = path.display().to_string();
    state.mode = Mode::File;
    state.file_scroll = 0;
    state.cursor = 0;
    Ok(())
}


pub fn render_file_viewer(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut State,
) -> Result<(), Box<dyn std::error::Error>> {
    terminal.draw(|f| {
        let size = f.size();
        let max_visible_lines = size.height.saturating_sub(2) as usize;

        // Adjust `file_scroll` to keep `cursor` within the visible range
        if state.cursor < state.file_scroll {
            state.file_scroll = state.cursor; // Scroll up
        } else if state.cursor >= state.file_scroll + max_visible_lines {
            state.file_scroll = state.cursor - max_visible_lines + 1; // Scroll down
        }

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

        let items: Vec<ListItem> = state
            .file_content
            .iter()
            .skip(state.file_scroll)
            .take(max_visible_lines)
            .map(|line| {
                create_line(line,state.cursor,state.show_lines)
                // if state.show_lines {
                //     ListItem::new(vec![create_line_with_number(line,state.cursor)])
                // } else {
                //     ListItem::new(vec![create_line_without_number(line,state.cursor)])
                // }
            })
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(state.cursor - state.file_scroll));

        let list = List::new(items)
            .block(file_block)
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD));

        f.render_stateful_widget(list, layout[0], &mut list_state);
    })?;
    Ok(())
}


// Helper function to create a line without a line number and styling
fn create_line(line: &Line,_line_index:usize,show_lines:bool) -> ListItem {
    let line_style = if line.is_selected {
        Style::default().fg(Color::Red)
    } else {
        Style::default()
    };

    let line_number_span = if show_lines {
            Span::styled(
            format!("{:<4}", line.line_number),
            Style::default().fg(Color::Blue),)
        } else {
            Span::raw("")
        }
    ;

    let line_content_span = Span::styled(line.content.clone(), line_style);
    ListItem::new(Spans::from(vec![line_number_span, line_content_span]))
}





pub fn handle_file_input(state: &mut State) -> Result<bool, io::Error> {
    if let Event::Key(KeyEvent { code, .. }) = event::read()? {
        match code {
            KeyCode::Char('q') => return Ok(true),

            KeyCode::Up  => {
                if state.cursor > 0 {
                    state.cursor -= 1;

                    // Scroll up if cursor is above the visible range
                    if state.cursor < state.file_scroll {
                        state.file_scroll = state.cursor;
                    }

                    state.asm_cursor=0;
                }
            }

            KeyCode::Char('w') => {
                if state.asm_cursor > 0 {
                    state.asm_cursor-=1;
                }
            }

            KeyCode::Char('s') => {
                state.asm_cursor+=1;
            }

            KeyCode::Down  => {
                if state.cursor < state.file_content.len().saturating_sub(1) {
                    state.cursor += 1;

                    // Scroll down if cursor goes below the visible range
                    let max_visible_lines = state.file_content.len().saturating_sub(1);
                    if state.cursor >= state.file_scroll + max_visible_lines {
                        state.file_scroll = state.cursor - max_visible_lines + 1;
                    }
                }

                state.asm_cursor=0;
            },

            KeyCode::Enter => {
                // Toggle selection of the current line under the cursor
                if let Some(line) = state.file_content.get_mut(state.cursor) {
                    line.is_selected = !line.is_selected;
                } else {
                    unreachable!();
                }
            }

            KeyCode::Char('l') => state.show_lines = !state.show_lines,

            KeyCode::Esc => state.mode = Mode::Dir,

            _ => {}
        }
    }
    Ok(false)
}


pub fn read_file_lines(path: &PathBuf) -> io::Result<Vec<Line>> {
    let file = File::open(path)?;
    let reader = io::BufReader::new(file);
    Ok(reader.lines().map_while(Result::ok)
        .enumerate()
        .map(|(i, s)|
            Line::new(s,i+1)
        )

        .collect())
}

// Helper function to create a line without a line number and styling
fn asm_create_line<'a>(line: &'a Line,show_lines:bool) -> ListItem<'a> {
    let line_style = if line.is_selected {
        Style::default()//.fg(Color::Red)
    } else {
        Style::default()
    };

    let line_number_span = if show_lines {
            Span::styled(
            format!("{:<4}", line.line_number),
            Style::default().fg(Color::Blue),)
        } else {
            Span::raw("")
        }
    ;

    let line_content_span = Span::styled(line.content.clone(), line_style);
    ListItem::new(Spans::from(vec![line_number_span, line_content_span]))
}

pub fn render_file_asm_viewer(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut State,
    code_file: &CodeFile,
    obj_path: Arc<Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    terminal.draw(|f| {
        let size = f.size();
        let max_visible_lines = size.height.saturating_sub(4) as usize;

        // Adjust `file_scroll` to keep `cursor` within the visible range
        if state.cursor < state.file_scroll {
            state.file_scroll = state.cursor;
        } else if state.cursor >= state.file_scroll + max_visible_lines {
            state.file_scroll = state.cursor - max_visible_lines + 1;
        }

        // Layout: Split vertically for source and assembly (if selected)
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
            .split(size);

        // Source file block
        let file_block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                format!("File Viewer - {}", state.file_path),
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
            ));

        let source_items: Vec<ListItem> = state
            .file_content
            .iter()
            .skip(state.file_scroll)
            .take(max_visible_lines)
            // .enumerate()
            .map(|line| {
                // let asm_list = make_assembly_inner(code_file.get_asm(&(line.line_number as u32),obj_path.clone()));
                asm_create_line(line,state.show_lines)//,asm_list)
            })
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(state.cursor - state.file_scroll));

        let list = List::new(source_items)
            .block(file_block)
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD));

        f.render_stateful_widget(list, layout[0], &mut list_state);

        // Assembly view block
        let asm_block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                "Assembly View",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));

        let asm_list = make_assembly_inner(
            code_file.get_asm(&( (state.cursor+1) as u32),obj_path),
            state
            );
        f.render_widget(asm_list.block(asm_block), layout[1]);
    })?;
    Ok(())
}

fn make_assembly_inner<'a>(op:Option<&'a [InstructionDetail]>,state:&'a mut State) -> List<'a>{
    match op {
        Some(instructions) => {
            let mut prev = -1isize;
            let mut asm_items = Vec::with_capacity(instructions.len());

            for ins in instructions.iter().skip(state.asm_cursor) {
                if ins.serial_number as isize != prev+1{
                    asm_items.push(
                        ListItem::new(
                        vec![Spans::from("DETATCH")]
                        )
                        .style(Style::default().fg(Color::Red))
                    )
            
                }
                prev = ins.serial_number as isize;
                let formatted_instruction = format!(
                    "{:#010x}: {:<6} {:<30}",
                    ins.address,
                    ins.mnemonic,
                    ins.op_str,
                );

                asm_items.push(ListItem::new(vec![Spans::from(formatted_instruction)])
                        .style(Style::default().fg(Color::Cyan)))

            }

            List::new(asm_items)
        },
        None => {
            let error_msg = vec![ListItem::new(Spans::from("No assembly instructions for this line."))];
            List::new(error_msg)
        }
    }
}