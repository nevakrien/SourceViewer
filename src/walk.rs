use crate::config::WalkConfig;
use crate::file_parser::InstructionDetail;
use crate::program_context::CodeFile;
use crate::program_context::CodeRegistry;
use core::cmp::min;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, MouseEvent,
    MouseEventKind,
};
use crossterm::execute;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io::{self};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
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

// static LAYOUT: [Constraint; 2] = [Constraint::Ratio(47,100), Constraint::Ratio(53,100)];

pub struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        execute!(io::stdout(), DisableMouseCapture).unwrap();
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
    }
}

pub struct GlobalState<'arena> {
    current_dir: Arc<Path>,
    // original_dir: Arc<Path>,
    pub dir_list_state: ListState,
    dir_entries: Box<[std::fs::DirEntry]>,
    layout: [Constraint; 2],
    show_lines: bool,

    selected_asm: BTreeMap<u64, (Cow<'arena, InstructionDetail>, Rc<str>)>, //address -> (instructions,line text)
    // asm_cursor: usize,
    cur_asm: u64,

    help_toggle: bool,
}

impl<'arena> GlobalState<'arena> {
    pub fn start() -> Result<Self, Box<dyn std::error::Error>> {
        //get the current dir so that .. works proper since ./.. is broken
        GlobalState::start_from(std::env::current_dir()?.into())
    }
    pub fn start_from(path: Arc<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let dir_entries = fs::read_dir(&*path)?.filter_map(Result::ok).collect();
        let config = WalkConfig::get_global()?;

        let mut state = Self {
            current_dir: path.clone(),
            // original_dir: path,
            dir_list_state: ListState::default(),
            // mode: Mode::Dir,
            // file_content: Vec::new(),
            dir_entries,
            layout: config.get_layout()?,

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
                self.selected_asm.extend(
                    data.iter()
                        .map(|x| (x.address, (Cow::Borrowed(x), text.clone()))),
                );
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

    // #[inline(always)]
    // fn cur_asm_range(
    //     &self,
    // ) -> std::collections::btree_map::Range<'_, u64, (Cow<'arena,InstructionDetail>, Rc<str>)> {
    //     // Start from asm_cursor and get all subsequent entries
    //     let start_key = self.selected_asm
    //     .range(..self.cur_asm)
    //     .rev()
    //     .take(2)
    //     .fold(self.cur_asm,|_,(a,_)| *a);

    //     self.selected_asm.range(start_key..)
    // }

    // #[inline]
    // fn asm_up(&mut self) {
    //     // Move to the previous address if possible
    //     if let Some(prev_address) = self
    //         .selected_asm
    //         .range(..self.cur_asm)
    //         .next_back()
    //         .map(|(addr, _)| *addr)
    //     {
    //         self.cur_asm = prev_address;
    //     }
    // }
    #[inline]
    fn asm_up(&mut self) {
        // Move to the previous address if possible
        if let Some((_, (prev_ins, _))) = self.selected_asm.range(..self.cur_asm).next_back() {
            if prev_ins.get_end() == self.cur_asm {
                self.cur_asm = prev_ins.address;
            } else {
                self.cur_asm = prev_ins.get_end();
            }
        }
    }

    // #[inline]
    // fn asm_down(&mut self) {
    //     // Move to the next address if possible
    //     if let Some(next_address) = self
    //         .selected_asm
    //         .range((self.cur_asm + 1)..)
    //         .next()
    //         .map(|(addr, _)| *addr)
    //     {
    //         self.cur_asm = next_address;
    //     }
    // }

    #[inline]
    fn asm_down(&mut self) {
        if let Some((addr, (ins, _))) = self.selected_asm.range((self.cur_asm)..).next() {
            if *addr == self.cur_asm {
                self.cur_asm = ins.get_end();
            } else {
                self.cur_asm = *addr;
            }
        }
    }

    #[inline]
    fn asm_toggle(
        &mut self,
        obj_path: &Path,
        code_files: &mut CodeRegistry<'_, 'arena>,
    ) -> Result<(), Box<dyn Error>> {
        use std::collections::btree_map::Entry;
        match self.selected_asm.entry(self.cur_asm) {
            Entry::Vacant(v) => {
                let machine_file = code_files.get_existing_machine(obj_path).unwrap();
                let ctx = machine_file.get_addr2line()?;

                let Some(raw_asm) = machine_file.dissasm_address(self.cur_asm)? else {
                    return Ok(());
                };

                if let Some(addr2line::Location {
                    file: Some(file),
                    line: Some(line),
                    ..
                }) = ctx.find_location(raw_asm.address)?
                {
                    let path = Path::new(file).into();
                    let code_file = code_files.get_source_file(path, false)?;
                    let text = code_file.get_line(line);
                    match text {
                        Some(t) => v.insert((
                            Cow::Owned(raw_asm),
                            sanitise(t.trim_start().to_string()).into(),
                        )),
                        None => v.insert((Cow::Owned(raw_asm), "<?>".into())),
                    };
                } else {
                    v.insert((Cow::Owned(raw_asm), "<?>".into()));
                }

                // Line::new(sanitise())
                self.asm_down();
            }
            Entry::Occupied(o) => {
                o.remove();
                self.asm_up();
            }
        };

        Ok(())
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
    command_input: String,
    command_mode: bool,
    // selected_asm: BTreeMap<u64,&'arena InstructionDetail>, //address -> instructions
}

impl<'me, 'arena> FileState<'me, 'arena> {
    #[inline]
    fn show_command_bar(&self) -> bool {
        self.command_mode || !self.command_input.is_empty()
    }

    #[inline]
    fn reset_command_bar(&mut self) {
        self.command_mode = false;
        self.command_input.clear();
    }

    #[inline]
    fn jump_to_line(&mut self, line: usize) {
        if self.file_content.is_empty() {
            return;
        }

        let max_index = self.file_content.len().saturating_sub(1);
        let target = line.saturating_sub(1).min(max_index);

        self.cursor = target;
        self.file_scroll = target;
    }

    #[inline]
    fn jump_to_address(
        &mut self,
        target_addr: u64,
        obj_path: &Path,
        code_files: &mut CodeRegistry<'_, 'arena>,
    ) -> Result<(), Box<dyn Error>> {
        // First check if address is already in selected_asm (including if it's in the middle of an instruction)
        for (addr, (ins, _)) in self.global.selected_asm.range(..=target_addr).rev() {
            if ins.address <= target_addr && target_addr < ins.get_end() {
                // Address is within this instruction
                self.global.cur_asm = *addr;
                return Ok(());
            }
        }

        // Try to find debug info for the target address
        let machine_file = code_files
            .get_existing_machine(obj_path)
            .ok_or("Failed to get machine file")?;
        let ctx = machine_file.get_addr2line()?;

        // Try direct lookup first
        if let Some(raw_asm) = machine_file.dissasm_address(target_addr)? {
            if let Some(addr2line::Location {
                file: Some(file),
                line: Some(line),
                ..
            }) = ctx.find_location(raw_asm.address)?
            {
                let path = Path::new(file).into();
                let code_file = code_files.get_source_file(path, false)?;
                let text = code_file.get_line(line);
                let text = match text {
                    Some(t) => sanitise(t.trim_start().to_string()).into(),
                    None => "<??>".into(),
                };
                self.global
                    .selected_asm
                    .insert(target_addr, (Cow::Owned(raw_asm), text));
                self.global.cur_asm = target_addr;
                return Ok(());
            }
        }

        // If direct lookup failed, iterate backwards to find a valid debug location
        const MAX_BACKTRACK: u64 = 10000; // Reasonable limit for backwards search
        for offset in 1..=MAX_BACKTRACK {
            let check_addr = target_addr.saturating_sub(offset);

            if let Some(raw_asm) = machine_file.dissasm_address(check_addr)? {
                if let Some(addr2line::Location {
                    file: Some(file),
                    line: Some(line),
                    ..
                }) = ctx.find_location(raw_asm.address)?
                {
                    let path = Path::new(file).into();
                    let code_file = code_files.get_source_file(path, false)?;
                    let text = code_file.get_line(line);
                    let text = match text {
                        Some(t) => sanitise(t.trim_start().to_string()).into(),
                        None => "<??>".into(),
                    };
                    self.global
                        .selected_asm
                        .insert(check_addr, (Cow::Owned(raw_asm), text));
                    self.global.cur_asm = check_addr;
                    return Ok(());
                }
            }
        }

        // If we couldn't find anything, fall back to the closest existing address
        if let Some((closest_addr, _)) = self.global.selected_asm.range(..=target_addr).next_back()
        {
            self.global.cur_asm = *closest_addr;
        } else if let Some((first_addr, _)) = self.global.selected_asm.iter().next() {
            self.global.cur_asm = *first_addr;
        }

        Ok(())
    }
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
    ) -> Result<Option<&'data [InstructionDetail]>, Box<dyn Error>> {
        // eprintln!("LOAD_DEBUG for line {} file {}", self.line_number, obj_path.display());

        match self.debug_info {
            Some(x) => Ok(x),
            None => {
                let ans = match code_file.get_asm(&(self.line_number as u32), obj_path) {
                    Some(res) => {
                        // eprintln!("LOAD_DEBUG found something");
                        Some(res?)
                    }
                    None => {
                        // eprintln!("LOAD_DEBUG found nothing!!!");
                        None
                    }
                };

                self.debug_info = Some(ans);
                Ok(self.debug_info.unwrap())
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
            .constraints(state.layout.as_ref())
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
    code_files: &mut CodeRegistry<'_, 'arena>,
    obj_path: Arc<Path>,
) -> Result<DirResult<'me, 'arena>, Box<dyn Error>> {
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
                    state.help_toggle = !state.help_toggle;
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
                KeyCode::Char(' ') => state.asm_toggle(&obj_path, code_files)?,

                KeyCode::Enter => {
                    if let Some(i) = state.dir_list_state.selected() {
                        let path: Arc<Path> = state.dir_entries[i].path().into();
                        if path.is_dir() {
                            state.current_dir = path;
                            load_dir(state)?;
                            state.dir_list_state.select(Some(0));
                        } else if path.is_file() {
                            let code_file = code_files.get_source_file(path.clone(), false)?;
                            return Ok(DirResult::File(load_file(state, &path, code_file)?));
                        }
                    }
                }
                KeyCode::Esc => {
                    // if state.current_dir != state.original_dir {
                    {
                        if let Some(parent) = state.current_dir.parent() {
                            let parent: Arc<Path> = parent.into();
                            state.current_dir = parent.clone();
                            state.dir_entries =
                                fs::read_dir(parent)?.filter_map(Result::ok).collect();
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
    code_file: &CodeFile,
) -> Result<FileState<'b, 'arena>, Box<dyn std::error::Error>> {
    Ok(FileState {
        file_content: read_file_lines(code_file),
        file_path: path.display().to_string(),
        file_scroll: 0,
        cursor: 0,
        // asm_cursor :0,
        global,
        command_input: String::new(),
        command_mode: false,
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
    code_files: &mut CodeRegistry<'_, 'arena>,
    code_file: &'arena CodeFile,
    obj_path: Arc<Path>,
) -> Result<FileResult, Box<dyn Error>> {
    match event::read()? {
        Event::Key(KeyEvent { code, kind, .. }) => {
            if crossterm::event::KeyEventKind::Release == kind {
                // Ignore key releases (We hate Windows!)
                return Ok(FileResult::KeepGoing);
            }

            if state.command_mode {
                match code {
                    KeyCode::Esc => state.reset_command_bar(),
                    KeyCode::Enter => {
                        let command = state.command_input.trim();
                        // Try parsing as hex address first
                        if let Some(hex_str) = command.strip_prefix("0x") {
                            if let Ok(addr) = u64::from_str_radix(&hex_str.to_lowercase(), 16) {
                                if let Err(_e) = state.jump_to_address(addr, &obj_path, code_files)
                                {
                                    // Silently handle errors - user will see if jump worked or not
                                }
                            }
                        } else if let Ok(line) = command.parse::<usize>() {
                            state.jump_to_line(line.max(1));
                        }
                        state.reset_command_bar();
                    }
                    KeyCode::Backspace => {
                        state.command_input.pop();
                        if state.command_input.is_empty() {
                            state.command_mode = false;
                        }
                    }
                    KeyCode::Char(c) => {
                        if !c.is_control() {
                            state.command_input.push(c);
                        }
                    }
                    _ => {}
                }

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
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    state.command_mode = true;
                    state.command_input.clear();
                    state.command_input.push(c);
                }
                KeyCode::Char(':') => {
                    state.command_mode = true;
                    state.command_input.clear();
                }
                KeyCode::Char('q') => return Ok(FileResult::Exit),
                KeyCode::Char('h') => {
                    state.global.help_toggle = true;
                }
                KeyCode::Char('w') => state.global.asm_up(),
                KeyCode::Char('s') => state.global.asm_down(),
                KeyCode::Char(' ') => state.global.asm_toggle(&obj_path, code_files)?,

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
                            state.global.add_asm_line(info?, line.content.clone())
                        } else {
                            state.global.remove_asm_line(info?)
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
    s = s.replace('\t', "  ");
    s.retain(|c| !c.is_control());

    s
}

fn read_file_lines(code_file: &CodeFile) -> Vec<Line<'static>> {
    // let file = File::open(path)?;
    // let reader = io::BufReader::new(file);

    code_file
        .text
        .lines()
        .enumerate()
        .map(|(i, s)| {
            // eprintln!("got {s:?}");
            let s = sanitise(s.to_string());
            // eprintln!("made {s:?}");

            Line::new(s.into(), i + 1)
        })
        .collect()
}

// Helper function to create a line without a line number and styling
fn create_line<'a>(line: &Line, show_lines: bool) -> ListItem<'a> {
    // eprintln!("displaying {:?}",line.content);

    let line_style = if line.is_selected {
        Style::default().fg(Color::Red) //.bg(Color::Rgb(50,0,0))
    } else {
        Style::default()
    };

    let line_number_span = if show_lines {
        Span::styled(
            format!("{:<4}", line.line_number),
            if line.is_selected {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Blue)
            },
        )
    } else {
        Span::raw("")
    };

    let line_text = match (&*line.content, line.is_selected) {
        ("", true) => "-".to_string(),
        (a, _) => a.to_string(),
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
        let mut layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(state.global.layout.as_ref())
            .split(size);

        let mut command_area = None;
        if state.show_command_bar() {
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(layout[0]);
            layout[0] = split[0];
            command_area = Some(split[1]);
        }

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

        if let Some(command_area) = command_area {
            let command_block = Block::default().borders(Borders::ALL).title(Span::styled(
                "Command",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));

            let command_text = format!("> {}", state.command_input);
            let command = Paragraph::new(command_text).block(command_block);
            f.render_widget(command, command_area);
        }

        if state.global.help_toggle {
            render_help_popup(f);
        }
    })?;

    Ok(())
}

fn maybe_highlight(h: bool, style: Style) -> Style {
    if h {
        style.bg(Color::DarkGray).add_modifier(Modifier::BOLD)
    } else {
        style
    }
}

fn make_assembly_inner<'a>(state: &GlobalState, max_visible_lines: usize) -> List<'a> {
    let asm_block = Block::default().borders(Borders::ALL).title(Span::styled(
        "Assembly View",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ));

    let mut asm_items = Vec::with_capacity(state.selected_asm.len());
    // asm_items.push(ListItem::new(vec![Spans::from(
    //     format!("currently holding {} items",state.selected_asm.len())
    // )]));

    let start_key = state
        .selected_asm
        .range(..state.cur_asm)
        .rev()
        .take((1 + max_visible_lines * 2) / 3)
        .fold(state.cur_asm, |_, (a, _)| *a);

    let mut iter = state.selected_asm.range(start_key..).peekable();

    let make_dots = |h: bool| {
        ListItem::new(vec![Spans::from("...")])
            .style(maybe_highlight(h, Style::default().fg(Color::Red)))
    };

    match iter.peek() {
        Some((0, _)) => {}
        Some(_) => {
            asm_items.push(make_dots(false));
        }
        None => {
            asm_items.push(make_dots(true));
        }
    }

    while let Some((_, (ins, text))) = iter.next() {
        if asm_items.len() >= max_visible_lines {
            break;
        }

        let formatted_instruction = format!(
            "{:#010x}: {:<6} {:<30} {:<30}",
            ins.address,
            ins.mnemonic,
            ins.op_str,
            text.trim_start(),
        );

        asm_items.push(
            ListItem::new(vec![Spans::from(formatted_instruction)]).style(maybe_highlight(
                ins.address == state.cur_asm,
                Style::default().fg(Color::Cyan),
            )),
        );

        if asm_items.len() >= max_visible_lines {
            break;
        }

        let Some((next_address, _)) = iter.peek() else {
            let selected = ins.get_end() <= state.cur_asm;
            asm_items.push(make_dots(selected));
            break;
        };

        let missing_range = ins.get_end()..**next_address;
        if !missing_range.is_empty() {
            let selected = missing_range.contains(&state.cur_asm);
            asm_items.push(make_dots(selected));
        }
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

            match handle_directory_input(state, code_files, obj_file.clone())? {
                DirResult::KeepGoing => {}
                DirResult::Exit => return Ok(()),
                DirResult::File(mut file_state) => {
                    let path: Arc<Path> = Path::new(&file_state.file_path).into();
                    // fs::canonicalize(Path::new(&file_state.file_path))?.into();
                    let code_file = code_files.get_source_file(path, true)?;
                    let res = Self::walk_file_loop(
                        &mut self.last_frame,
                        terminal,
                        &mut file_state,
                        code_files,
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
        code_files: &mut CodeRegistry<'_, 'arena>,
        code_file: &'arena CodeFile,
        obj_file: Arc<Path>,
    ) -> Result<FileResult, Box<dyn std::error::Error>> {
        loop {
            wait_frame_start(last_frame)?;

            render_file_asm_viewer(terminal, file_state)?;
            let res = handle_file_input(file_state, code_files, code_file, obj_file.clone())?;
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
        "  Space      - Toggle selection of the current address",
        "              and load/unload associated assembly",
        "",
        "Command Bar:",
        "  0-9        - Start a command (numbers jump to a line)",
        "  :          - Also opens the command bar",
        "  Enter      - Run command and close",
        "  Backspace  - Delete, closing when empty",
        "  Esc        - Close the command bar without running",
        "",
        "Other Commands:",
        "  h          - Show this",
        "  q          - Quit file viewer",
        "  l          - Toggle line numbers",
        "  Esc        - Return to directory view",
    ];

    render_popup(frame, "Help", &help_content, 80, 80); // 80% width, 80% height
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
        "  Space      - Toggle selection of the current address",
        "              and load/unload associated assembly",
        "",
        "Other Commands:",
        "  h          - Show this help",
        "  q          - Quit the application",
    ];

    render_popup(frame, "Help", &help_content, 80, 80); // 80% width, 80% height
}
