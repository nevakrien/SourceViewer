use crate::file_parser::InstructionDetail;
use crate::program_context::CodeFile;
use crate::program_context::CodeRegistry;
use core::cmp::min;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, MouseEvent,
    MouseEventKind,
};
use crossterm::execute;
use std::collections::BTreeMap;
use std::io::{self, BufRead};
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use std::{fs, fs::File};
use tui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};

const ACTIONS_PER_SECOND: u64 = 30; // Frames per second for terminal updates
const FRAME_MIN_TIME: Duration = Duration::from_millis(1000 / ACTIONS_PER_SECOND);

pub struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        execute!(io::stdout(), DisableMouseCapture).unwrap();
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
    }
}

pub struct GlobalState<'arena> {
    current_dir: PathBuf,
    original_dir: PathBuf,
    pub dir_list_state: ListState,
    dir_entries: Box<[std::fs::DirEntry]>,

    show_lines: bool,

    selected_asm: BTreeMap<u64, (&'arena InstructionDetail, Rc<str>)>, //address -> (instructions,line text)
    // asm_cursor: usize,
    cur_asm: u64,

    help_toggle: bool,
}

impl<'arena> GlobalState<'arena> {
    pub fn start() -> Result<Self, Box<dyn std::error::Error>> {
        GlobalState::start_from(PathBuf::from("."))
    }
    pub fn start_from(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let dir_entries = fs::read_dir(&*path)?.filter_map(Result::ok).collect();
        let mut state = Self {
            current_dir: path.clone(),
            original_dir: path,
            dir_list_state: ListState::default(),
            // mode: Mode::Dir,
            // file_content: Vec::new(),
            dir_entries,

            // file_scroll: 0,
            // cursor: 0,
            // file_path: String::new(),
            show_lines: false,
            selected_asm: BTreeMap::new(),

            // asm_cursor:0,
            cur_asm: 0,

            help_toggle: false,
            // asm_lines: BTreeMap::default()
        };

        state.dir_list_state.select(Some(0)); // Initialize the selected index
        Ok(state)
    }

    fn add_asm_line(&mut self, debug: Option<&'arena [InstructionDetail]>, text: Rc<str>) {
        match debug {
            None => {}
            Some(data) => {
                // let current_addres = self
                self.selected_asm
                    .extend(data.iter().map(|x| (x.address, (x, text.clone()))));
            }
        }
    }

    fn remove_asm_line(&mut self, debug: Option<&'arena [InstructionDetail]>) {
        for address in debug.unwrap_or_default().iter().map(|x| x.address) {
            self.selected_asm.remove(&address);
        }
        self.cur_asm = min(
            self.cur_asm,
            self.selected_asm
                .last_key_value()
                .map(|(k, _)| *k)
                .unwrap_or_default(),
        );
    }

    #[inline(always)]
    fn cur_asm_range(
        &self,
    ) -> std::collections::btree_map::Range<'_, u64, (&'arena InstructionDetail, Rc<str>)> {
        // Start from asm_cursor and get all subsequent entries
        self.selected_asm.range(self.cur_asm..)
    }

    #[inline]
    fn asm_up(&mut self) {
        // Move to the previous address if possible
        if let Some(prev_address) = self
            .selected_asm
            .range(..self.cur_asm)
            .next_back()
            .map(|(addr, _)| *addr)
        {
            self.cur_asm = prev_address;
        }
    }

    #[inline]
    fn asm_down(&mut self) {
        // Move to the next address if possible
        if let Some(next_address) = self
            .selected_asm
            .range((self.cur_asm + 1)..)
            .next()
            .map(|(addr, _)| *addr)
        {
            self.cur_asm = next_address;
        }
    }

    // #[inline(always)]
    // fn cur_asm_range(&self)  -> impl Iterator<Item = &&InstructionDetail> {
    //     self.selected_asm.iter().map(|(_,x)| x).skip(self.asm_cursor)
    // }

    // #[inline]
    // fn asm_up(&mut self) {
    //     if self.asm_cursor > 0 {
    //         self.asm_cursor-=1;
    //     }
    // }

    // #[inline]
    // fn asm_down(&mut self) {
    //     if self.asm_cursor < self.selected_asm.len().saturating_sub(1){
    //         self.asm_cursor+=1;
    //     }
    // }
}

pub struct FileState<'me, 'arena> {
    // pub current_dir: PathBuf,
    // pub original_dir: PathBuf,
    // pub dir_list_state: ListState,
    // pub mode: Mode,
    file_content: Vec<Line<'arena>>,

    pub file_scroll: usize,
    pub cursor: usize,
    pub file_path: String,

    global: &'me mut GlobalState<'arena>,
    // selected_asm: BTreeMap<u64,&'arena InstructionDetail>, //address -> instructions
}

struct Line<'data> {
    content: Rc<str>,
    is_selected: bool,
    line_number: usize, // Optionally store the line number
    debug_info: Option<Option<&'data [InstructionDetail]>>, // debug_info: Option<String>,  // Placeholder for future debug information
}

impl<'data> Line<'data> {
    fn new(content: Rc<str>, line_number: usize) -> Self {
        Self {
            content,
            is_selected: false,
            line_number,
            debug_info: None,
        }
    }

    #[inline(always)]
    fn load_debug(
        &mut self,
        code_file: &'data CodeFile,
        obj_path: Arc<Path>,
    ) -> Option<&'data [InstructionDetail]> {
        match self.debug_info {
            Some(x) => x,
            None => {
                self.debug_info = Some(code_file.get_asm(&(self.line_number as u32), obj_path));
                self.debug_info.unwrap()
            }
        }
    }
}

pub fn create_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, io::Error> {
    execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
    crossterm::terminal::enable_raw_mode()?;
    execute!(io::stdout(), EnableMouseCapture)?;
    let backend = CrosstermBackend::new(io::stdout());
    Terminal::new(backend)
}

pub fn load_dir(state: &mut GlobalState) -> Result<(), Box<dyn std::error::Error>> {
    state.dir_entries = fs::read_dir(&state.current_dir)?
        .filter_map(Result::ok)
        .collect();
    Ok(())
}

pub fn render_directory(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut GlobalState,
) -> Result<(), Box<dyn std::error::Error>> {
    let items: Vec<ListItem> = state
        .dir_entries
        .iter()
        .map(|entry| {
            let name = entry.file_name().into_string().unwrap_or_default();
            ListItem::new(Span::styled(name, Style::default().fg(Color::White)))
        })
        .collect();

    terminal.draw(|f| {
        let size = f.size();

        // Layout: Split vertically for source and assembly (if selected)
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
            .split(size);

        let list_block = Block::default().borders(Borders::ALL).title(Span::styled(
            format!("Directory Browser - {}", state.current_dir.display()),
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ));
        let list = List::new(items)
            .block(list_block)
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        f.render_stateful_widget(list, layout[0], &mut state.dir_list_state);
        let asm_lines = layout[1].height.saturating_sub(2) as usize;
        f.render_widget(make_assembly_inner(state, asm_lines), layout[1]);

        if state.help_toggle {
            render_dir_help_popup(f)
        }
    })?;
    Ok(())
}

pub enum DirResult<'me, 'arena> {
    KeepGoing,
    File(FileState<'me, 'arena>),
    Exit,
}

pub fn handle_directory_input<'me, 'arena>(
    state: &'me mut GlobalState<'arena>,
) -> Result<DirResult<'me, 'arena>, Box<dyn std::error::Error>> {
    match event::read()? {
        Event::Key(KeyEvent { code, kind, .. }) => {
            if crossterm::event::KeyEventKind::Release == kind {
                // Ignore key releases (We hate Windows!)
                return Ok(DirResult::KeepGoing);
            }
            match code {
                KeyCode::Char('q') => {
                    // return Err("exiting normally".into());
                    return Ok(DirResult::Exit);
                }
                KeyCode::Char('h') => {
                    state.help_toggle = true;
                }
                KeyCode::Down => {
                    let i = match state.dir_list_state.selected() {
                        Some(i) => (i + 1) % state.dir_entries.len(),
                        None => 0,
                    };
                    state.dir_list_state.select(Some(i));
                }
                KeyCode::Up => {
                    let i = match state.dir_list_state.selected() {
                        Some(i) => {
                            if i == 0 {
                                state.dir_entries.len() - 1
                            } else {
                                i - 1
                            }
                        }
                        None => 0,
                    };
                    state.dir_list_state.select(Some(i));
                }

                KeyCode::Char('w') => state.asm_up(),

                KeyCode::Char('s') => state.asm_down(),

                KeyCode::Enter => {
                    if let Some(i) = state.dir_list_state.selected() {
                        let path = state.dir_entries[i].path();
                        if path.is_dir() {
                            state.current_dir = path;
                            load_dir(state)?;
                            state.dir_list_state.select(Some(0));
                        } else if path.is_file() {
                            return Ok(DirResult::File(load_file(state, &path)?));
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
        Event::Mouse(MouseEvent { kind, .. }) => {
            match kind {
                MouseEventKind::ScrollDown => {
                    let i = match state.dir_list_state.selected() {
                        Some(i) => (i + 1) % state.dir_entries.len(),
                        None => 0,
                    };
                    state.dir_list_state.select(Some(i));
                }
                MouseEventKind::ScrollUp => {
                    let i = match state.dir_list_state.selected() {
                        Some(i) => {
                            if i == 0 {
                                state.dir_entries.len() - 1
                            } else {
                                i - 1
                            }
                        }
                        None => 0,
                    };
                    state.dir_list_state.select(Some(i));
                }
                _ => {}
            }
            return Ok(DirResult::KeepGoing);
        }
        _ => {}
    }

    Ok(DirResult::KeepGoing)
}

pub fn load_file<'b, 'arena>(
    global: &'b mut GlobalState<'arena>,
    path: &Path,
) -> Result<FileState<'b, 'arena>, Box<dyn std::error::Error>> {
    Ok(FileState {
        file_content: read_file_lines(path)?,
        file_path: path.display().to_string(),
        file_scroll: 0,
        cursor: 0,
        // asm_cursor :0,
        global,
        // selected_asm: BTreeMap::new(),
    })
}

pub enum FileResult {
    Dir,
    KeepGoing,
    Exit,
}

//code_file: &'arena CodeFile,obj_path: Arc<Path>
pub fn handle_file_input<'arena>(
    state: &mut FileState<'_, 'arena>,
    code_file: &'arena CodeFile,
    obj_path: Arc<Path>,
) -> Result<FileResult, io::Error> {
    match event::read()? {
        Event::Key(KeyEvent { code, kind, .. }) => {
            if crossterm::event::KeyEventKind::Release == kind {
                // Ignore key releases (We hate Windows!)
                return Ok(FileResult::KeepGoing);
            }

            if state.global.help_toggle {
                if code == KeyCode::Char('h') {
                    state.global.help_toggle = false;
                } else if code == KeyCode::Char('q') {
                    return Ok(FileResult::Exit);
                }
                return Ok(FileResult::KeepGoing);
            }

            match code {
                KeyCode::Char('q') => return Ok(FileResult::Exit),
                KeyCode::Char('h') => {
                    state.global.help_toggle = true;
                }
                KeyCode::Char('w') => state.global.asm_up(),
                KeyCode::Char('s') => state.global.asm_down(),
                KeyCode::Up => {
                    if state.cursor > 0 {
                        state.cursor -= 1;

                        // Scroll up if cursor is above the visible range
                        if state.cursor < state.file_scroll {
                            state.file_scroll = state.cursor;
                        }
                    }
                }
                KeyCode::Down => {
                    if state.cursor < state.file_content.len().saturating_sub(1) {
                        state.cursor += 1;

                        // Scroll down if cursor goes below the visible range
                        let max_visible_lines = state.file_content.len().saturating_sub(1);
                        if state.cursor >= state.file_scroll + max_visible_lines {
                            state.file_scroll = state.cursor - max_visible_lines + 1;
                        }
                    }
                }
                KeyCode::Enter => {
                    // Toggle selection of the current line under the cursor
                    if let Some(line) = state.file_content.get_mut(state.cursor) {
                        line.is_selected = !line.is_selected;
                        let info = line.load_debug(code_file, obj_path);

                        if line.is_selected {
                            state.global.add_asm_line(info, line.content.clone())
                        } else {
                            state.global.remove_asm_line(info)
                        }
                    } else {
                        unreachable!();
                    }
                }
                KeyCode::Char('l') => state.global.show_lines = !state.global.show_lines,
                KeyCode::Esc => {
                    return Ok(FileResult::Dir);
                }

                _ => {}
            }
        }
        Event::Mouse(MouseEvent { kind, .. }) => {
            match kind {
                MouseEventKind::ScrollDown => {
                    if state.cursor < state.file_content.len().saturating_sub(1) {
                        state.cursor += 1;

                        // Scroll down if cursor goes below the visible range
                        let max_visible_lines = state.file_content.len().saturating_sub(1);
                        if state.cursor >= state.file_scroll + max_visible_lines {
                            state.file_scroll = state.cursor - max_visible_lines + 1;
                        }
                    }
                }
                MouseEventKind::ScrollUp => {
                    if state.cursor > 0 {
                        state.cursor -= 1;

                        // Scroll up if cursor is above the visible range
                        if state.cursor < state.file_scroll {
                            state.file_scroll = state.cursor;
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(FileResult::KeepGoing)
}

/// make text consistently renderble
fn sanitise(mut s: String) -> String {
    s.retain(|c| !c.is_control());
    s = s.replace('\t', "  ");
    s
}

fn read_file_lines(path: &Path) -> io::Result<Vec<Line<'static>>> {
    let file = File::open(path)?;
    let reader = io::BufReader::new(file);

    Ok(reader
        .lines()
        .map_while(Result::ok)
        .enumerate()
        .map(|(i, s)| Line::new(sanitise(s).into(), i + 1))
        .collect())
}

// Helper function to create a line without a line number and styling
fn create_line<'a>(line: &Line, show_lines: bool) -> ListItem<'a> {
    let line_style = if line.is_selected {
        Style::default().fg(Color::Red) //.bg(Color::Rgb(50,0,0))
    } else {
        Style::default()
    };

    let line_number_span = if show_lines {
        Span::styled(
            format!("{:<4}", line.line_number),
            Style::default().fg(Color::Blue),
        )
    } else {
        Span::raw("")
    };

    let line_text = match &*line.content {
        "" => "-".to_string(),
        a => a.to_string(),
    };

    let line_content_span = Span::styled(line_text, line_style);
    ListItem::new(Spans::from(vec![line_number_span, line_content_span]))
}

pub fn render_file_asm_viewer(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut FileState,
    // code_file: &CodeFile,
    // obj_path: Arc<Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    terminal.draw(|f| {
        clear_entire_screen(f);

        let size = f.size();

        // Layout: Split vertically for source and assembly (if selected)
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
            .split(size);

        // Calculate max visible lines based on the height of the first part of the layout
        let max_visible_lines = layout[0].height.saturating_sub(2) as usize;

        // Adjust `file_scroll` to keep `cursor` within the visible range for the first layout section
        if state.cursor < state.file_scroll {
            state.file_scroll = state.cursor; // Scroll up
        } else if state.cursor >= state.file_scroll + max_visible_lines {
            state.file_scroll = state.cursor - max_visible_lines + 1; // Scroll down
        }

        // Source file block
        let file_block = Block::default().borders(Borders::ALL).title(Span::styled(
            format!("File Viewer - {}", state.file_path),
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ));

        let source_items: Vec<ListItem> = state
            .file_content
            .iter()
            .skip(state.file_scroll)
            .take(max_visible_lines)
            // .enumerate()
            .map(|line| {
                // let asm_list = make_assembly_inner(code_file.get_asm(&(line.line_number as u32),obj_path.clone()));
                create_line(line, state.global.show_lines) //,asm_list)
            })
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(state.cursor - state.file_scroll));

        let list = List::new(source_items).block(file_block).highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

        f.render_stateful_widget(list, layout[0], &mut list_state);

        let asm_lines = layout[1].height.saturating_sub(2) as usize;
        f.render_widget(make_assembly_inner(state.global, asm_lines), layout[1]);

        if state.global.help_toggle {
            render_help_popup(f);
        }
    })?;

    Ok(())
}

fn make_assembly_inner<'a>(state: &GlobalState, max_visible_lines: usize) -> List<'a>
// op:Option<I>,
 // where I: Iterator<Item = &'a InstructionDetail> + ExactSizeIterator ,
{
    let asm_block = Block::default().borders(Borders::ALL).title(Span::styled(
        "Assembly View",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ));

    // let mut prev = -1isize;
    let mut prev_end = 0u64;

    // let mut asm_items = Vec::new();
    let mut asm_items = Vec::with_capacity(state.selected_asm.len());

    for (ins, text) in state
        .cur_asm_range()
        .map(|(_, v)| v.clone())
        .take(max_visible_lines)
    {
        // if ins.serial_number as isize != prev + 1 {
        if ins.address != prev_end {
            asm_items.push(
                ListItem::new(vec![Spans::from("...")]).style(Style::default().fg(Color::Red)),
            )
        }
        prev_end = ins.get_end();
        

        //print
        // prev = ins.serial_number as isize;
        let formatted_instruction = format!(
            // "{:<4} {:#010x}: {:<6} {:<30} {:<30}",
            // ins.serial_number,
            "{:#010x}: {:<6} {:<30} {:<30}",

            ins.address,
            ins.mnemonic,
            ins.op_str,
            text.trim_start(),
        );

        asm_items.push(
            ListItem::new(vec![Spans::from(formatted_instruction)])
                .style(Style::default().fg(Color::Cyan)),
        )
    }

    List::new(asm_items).block(asm_block)
    // .highlight_style(Style::default()
    // .bg(Color::DarkGray)
    // .add_modifier(Modifier::BOLD))
}

pub struct TerminalSession<'me, 'arena> {
    pub terminal: Terminal<CrosstermBackend<io::Stdout>>,
    _cleanup: TerminalCleanup, // Ensures cleanup lasts as long as TerminalSession
    pub state: &'me mut GlobalState<'arena>,
    last_frame: Instant,
}

pub fn wait_frame_start(last_frame: &mut Instant) -> Result<(), Box<dyn std::error::Error>> {
    let mut now = Instant::now();

    // eprintln!("start waiting frame");

    //busy loop since asking for time causes an OS switch anyway
    //BUT we wana flush events ASAP
    while now - *last_frame < FRAME_MIN_TIME {
        //puting a proper poll breaks some functionality for stupid reason
        //this is likely a bug in the terminal libarary
        while event::poll(Duration::ZERO)? {
            let _ = event::read();
        }
        now = Instant::now();
    }

    *last_frame = now;
    // eprintln!("done waiting frame");
    Ok(())
}

impl<'me, 'arena> TerminalSession<'me, 'arena> {
    // Initialize Terminal, GlobalState, and TerminalCleanup
    pub fn new(state: &'me mut GlobalState<'arena>) -> Result<Self, Box<dyn std::error::Error>> {
        let terminal = create_terminal()?;
        let cleanup = TerminalCleanup;
        let last_frame = Instant::now() - Duration::from_secs(100); //literally just there as filler
                                                                    // let state = GlobalState::start()?;
        Ok(Self {
            terminal,
            _cleanup: cleanup,
            state,
            last_frame,
        })
    }

    // Directory loop to select files
    pub fn walk_directory_loop(
        &mut self,
        code_files: &mut CodeRegistry<'_, 'arena>,
        obj_file: Arc<Path>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            wait_frame_start(&mut self.last_frame)?;

            render_directory(&mut self.terminal, self.state)?;

            let terminal = &mut self.terminal;
            let state = &mut self.state;

            match handle_directory_input(state)? {
                DirResult::KeepGoing => {}
                DirResult::Exit => return Ok(()),
                DirResult::File(mut file_state) => {
                    let path: Arc<Path> =
                        fs::canonicalize(Path::new(&file_state.file_path))?.into();
                    let code_file = code_files.get_source_file(path,false)?;
                    let res = Self::walk_file_loop(
                        &mut self.last_frame,
                        terminal,
                        &mut file_state,
                        code_file,
                        obj_file.clone(),
                    )?;

                    match res {
                        FileResult::Exit => return Ok(()),
                        FileResult::Dir => {}
                        FileResult::KeepGoing => unreachable!(),
                    }
                }
            }
        }
    }
    // File loop to display and navigate files
    pub fn walk_file_loop(
        last_frame: &mut Instant,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        file_state: &mut FileState<'_, 'arena>,
        code_file: &'arena CodeFile,
        obj_file: Arc<Path>,
    ) -> Result<FileResult, Box<dyn std::error::Error>> {
        loop {
            wait_frame_start(last_frame)?;

            render_file_asm_viewer(terminal, file_state)?;
            let res = handle_file_input(file_state, code_file, obj_file.clone())?;
            match res {
                FileResult::KeepGoing => {}
                _ => return Ok(res),
            }
        }
    }
}

use tui::widgets::Clear;
pub fn clear_entire_screen<B: tui::backend::Backend>(frame: &mut tui::Frame<B>) {
    let entire_area = frame.size(); // Get the entire terminal size
    frame.render_widget(Clear, entire_area);
}

pub fn render_popup(
    frame: &mut Frame<CrosstermBackend<io::Stdout>>,
    title: &str,
    content: &[&str],
    width_percent: u16,
    height_percent: u16,
) {
    // Calculate centered area
    let terminal_size = frame.size();
    let popup_width = terminal_size.width * width_percent / 100;
    let popup_height = terminal_size.height * height_percent / 100;
    let popup_x = terminal_size.x + (terminal_size.width - popup_width) / 2;
    let popup_y = terminal_size.y + (terminal_size.height - popup_height) / 2;
    let area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the popup area
    frame.render_widget(Clear, area);

    // Create the popup block
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White));

    let paragraph = Paragraph::new(
        content
            .iter()
            .map(|line| Spans::from(*line))
            .collect::<Vec<_>>(),
    )
    .block(block)
    .alignment(Alignment::Left); // Use left alignment for help content

    // Render the popup
    frame.render_widget(paragraph, area);
}

pub fn render_help_popup(frame: &mut Frame<CrosstermBackend<io::Stdout>>) {
    let help_content = [
        "Help - File Input Behavior",
        "",
        "Navigation:",
        "  Up Arrow   - Move cursor up",
        "  Down Arrow - Move cursor down",
        "",
        "Selection:",
        "  Enter      - Toggle selection of the current line",
        "              and load/unload associated assembly",
        "",
        "Assembly View:",
        "  w          - Scroll assembly view up",
        "  s          - Scroll assembly view down",
        "  l          - Toggle line numbers",
        "",
        "Other Commands:",
        "  h          - Show this",
        "  q          - Quit file viewer",
        "  Esc        - Return to directory view",
    ];

    render_popup(frame, "Help", &help_content, 50, 50); // 50% width, 50% height
}

pub fn render_dir_help_popup(frame: &mut Frame<CrosstermBackend<io::Stdout>>) {
    let help_content = [
        "Help - Directory Navigation",
        "",
        "Navigation:",
        "  Up Arrow   - Move selection up",
        "  Down Arrow - Move selection down",
        "",
        "Directory Actions:",
        "  Enter      - Open selected directory or file",
        "  Esc        - Navigate to parent directory",
        "",
        "Assembly View:",
        "  w          - Scroll assembly view up",
        "  s          - Scroll assembly view down",
        "",
        "Other Commands:",
        "  h          - Show this help",
        "  q          - Quit the application",
    ];

    render_popup(frame, "Help", &help_content, 50, 50); // 50% width, 50% height
}
