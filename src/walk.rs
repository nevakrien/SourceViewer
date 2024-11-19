use core::cmp::min;
use std::collections::BTreeMap;
use std::collections::btree_map;
use std::fs;
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

#[derive(PartialEq,Clone,Debug)]
pub enum Mode {
    Dir,
    File,
}

pub struct GlobalState<'arena> {
    current_dir: PathBuf,
    original_dir: PathBuf,
    pub dir_list_state: ListState,
    dir_entries: Box<[std::fs::DirEntry]>,

    show_lines: bool,

    selected_asm: BTreeMap<u64,&'arena InstructionDetail>, //address -> instructions
    // asm_cursor: usize,
    cur_asm: u64,



}




impl<'arena> GlobalState<'arena> {
    pub fn start() -> Result<Self, Box<dyn std::error::Error>>  {
        let dir_entries = fs::read_dir(".")?
                    .filter_map(Result::ok)
                    .collect();
        Ok(Self {
            current_dir: PathBuf::from("."),
            original_dir: PathBuf::from("."),
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
            cur_asm:0,

            // asm_lines: BTreeMap::default()
        })
    }

    fn add_asm_line(&mut self,debug:Option<&'arena [InstructionDetail]>) {
        match debug {
            None=>{},
            Some(data)=>{
                // let current_addres = self
                self.selected_asm.extend(
                data.iter().map(|x| 
                    (x.address,x)
                    )                
                );
            }
        }
        
    }

    fn remove_asm_line(&mut self,debug:Option<&'arena [InstructionDetail]>) {
        for address in debug.unwrap_or_default().iter().map(|x| x.address){
            self.selected_asm.remove(&address);
        }
        self.cur_asm = min(self.cur_asm,
            self.selected_asm
            .last_key_value()
            .map(|(k,_)| *k)
            .unwrap_or_default()
        );
        
    }

     #[inline(always)]
    fn cur_asm_range(&self) -> std::collections::btree_map::Range<'_, u64, &'arena InstructionDetail> {
        // Start from asm_cursor and get all subsequent entries
        self.selected_asm.range(self.cur_asm..)
    }

    #[inline]
    fn asm_up(&mut self) {
        // Move to the previous address if possible
        if let Some(prev_address) = self.selected_asm.range(..self.cur_asm).next_back().map(|(addr, _)| *addr) {
            self.cur_asm = prev_address;
        }
    }

    #[inline]
    fn asm_down(&mut self) {
        // Move to the next address if possible
        if let Some(next_address) = self.selected_asm.range((self.cur_asm + 1)..).next().map(|(addr, _)| *addr) {
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


pub struct FileState<'me,'arena> {

    // pub current_dir: PathBuf,
    // pub original_dir: PathBuf,
    // pub dir_list_state: ListState,
    // pub mode: Mode,
    file_content: Vec<Line<'arena>>,
    

    file_scroll: usize,
    cursor: usize,
    pub file_path: String,


    global: &'me mut GlobalState<'arena>,

    // selected_asm: BTreeMap<u64,&'arena InstructionDetail>, //address -> instructions

}

struct Line<'data> {
    content: String,
    is_selected: bool,
    line_number: usize, // Optionally store the line number
    debug_info: Option<Option<&'data [InstructionDetail]>>
    // debug_info: Option<String>,  // Placeholder for future debug information
}

impl<'data> Line<'data> {
    fn new(content: String, line_number: usize) -> Self {
        Self {
            content,
            is_selected: false,
            line_number,
            debug_info: None,
        }
    }

    #[inline(always)]
    fn load_debug(&mut self,code_file:&'data CodeFile,obj_path:Arc<Path>) -> Option<&'data [InstructionDetail]> {
        match self.debug_info{
            Some(x) => x,
            None => {
                self.debug_info = Some(code_file.get_asm(&(self.line_number as u32),obj_path));
                self.debug_info.unwrap()

            }
        }

    }
}


pub fn create_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, io::Error> {
    execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
    crossterm::terminal::enable_raw_mode()?;
    let backend = CrosstermBackend::new(io::stdout());
    Terminal::new(backend)
}

pub fn load_dir(state:&mut GlobalState) -> Result<(), Box<dyn std::error::Error>> {
    state.dir_entries = fs::read_dir(&state.current_dir)?
                    .filter_map(Result::ok)
                    .collect();
    Ok(())
}

pub fn render_directory(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut GlobalState,
) -> Result<(), Box<dyn std::error::Error>> {
    let items: Vec<ListItem> = state.dir_entries
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
        f.render_widget(make_assembly_inner(state), layout[1]);
    })?;
    Ok(())
}

pub enum DirResult<'me,'arena> {
    
    KeepGoing,
    File(FileState<'me, 'arena>),
    Exit
}

pub fn handle_directory_input<'me,'arena>(
    state: &'me mut GlobalState<'arena>,
) -> Result<DirResult<'me,'arena>, Box<dyn std::error::Error>> {
    if let Event::Key(KeyEvent { code, .. }) = event::read()? {
        match code {
            KeyCode::Char('q') => return Ok(DirResult::Exit),
            KeyCode::Down  => {
                let i = match state.dir_list_state.selected() {
                    Some(i) => (i + 1) % state.dir_entries.len(),
                    None => 0,
                };
                state.dir_list_state.select(Some(i));
            }
            KeyCode::Up  => {
                let i = match state.dir_list_state.selected() {
                    Some(i) => if i == 0 { state.dir_entries.len() - 1 } else { i - 1 },
                    None => 0,
                };
                state.dir_list_state.select(Some(i));
            }

            KeyCode::Char('w') => {
                state.asm_up()
            }

            KeyCode::Char('s') => {
                state.asm_down()
            }

            KeyCode::Enter => {
                if let Some(i) = state.dir_list_state.selected() {
                    let path = state.dir_entries[i].path();
                    if path.is_dir() {
                        state.current_dir = path;
                        load_dir(state)?;
                        state.dir_list_state.select(Some(0));
                    } else if path.is_file() {
                        return Ok(DirResult::File(load_file(state,&path)?));

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
    Ok(DirResult::KeepGoing)
}



pub fn load_file<'b,'arena>(global:&'b mut GlobalState<'arena>, path:&PathBuf) -> Result<FileState<'b,'arena>, Box<dyn std::error::Error>> {
    Ok(FileState{
    file_content :read_file_lines(path)?,
    file_path : path.display().to_string(),
    file_scroll : 0,
    cursor : 0,
    // asm_cursor :0,
    global,
    // selected_asm: BTreeMap::new(),
    })
}

pub enum FileResult {
    Dir,
    KeepGoing,
    Exit
}

//code_file: &'arena CodeFile,obj_path: Arc<Path>
pub fn handle_file_input<'arena>(state: &mut FileState<'_,'arena>,code_file: &'arena CodeFile,obj_path: Arc<Path> ) -> Result<FileResult, io::Error> {
    if let Event::Key(KeyEvent { code, .. }) = event::read()? {
        match code {
            KeyCode::Char('q') => return Ok(FileResult::Exit),
            KeyCode::Char('w') => {
                state.global.asm_up()
            }

            KeyCode::Char('s') => {
                state.global.asm_down()
            }

            KeyCode::Up  => {
                if state.cursor > 0 {
                    state.cursor -= 1;

                    // Scroll up if cursor is above the visible range
                    if state.cursor < state.file_scroll {
                        state.file_scroll = state.cursor;
                    }

                }
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

            },

            KeyCode::Enter => {
                // Toggle selection of the current line under the cursor
                if let Some(line) = state.file_content.get_mut(state.cursor) {
                    line.is_selected = !line.is_selected;
                    let info = line.load_debug(code_file,obj_path);


                    if line.is_selected{    
                        state.global.add_asm_line(info)
                    }else {
                        state.global.remove_asm_line(info)

                    }

                } else {
                    unreachable!();
                }
            }

            KeyCode::Char('l') => state.global.show_lines = !state.global.show_lines,

            KeyCode::Esc => {return Ok(FileResult::Dir);},

            _ => {}
        }
    }
    Ok(FileResult::KeepGoing)
}


fn read_file_lines(path: &PathBuf) -> io::Result<Vec<Line<'static>>> {
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
fn create_line<'a>(line: &Line,show_lines:bool) -> ListItem<'a> {
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



pub fn render_file_asm_viewer(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut FileState,
    code_file: &CodeFile,
    obj_path: Arc<Path>,

) -> Result<(), Box<dyn std::error::Error>> {
    terminal.draw(|f| {
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
                create_line(line,state.global.show_lines)//,asm_list)
            })
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(state.cursor - state.file_scroll));

        let list = List::new(source_items)
            .block(file_block)
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD));

        f.render_stateful_widget(list, layout[0], &mut list_state);


        f.render_widget(make_assembly_inner(state.global), layout[1]);
    })?;
    Ok(())
}

fn make_assembly_inner<'a>(state: &GlobalState) -> List<'a>
 // op:Option<I>,
 // where I: Iterator<Item = &'a InstructionDetail> + ExactSizeIterator ,
 {
    let asm_block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                "Assembly View",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));

    let mut prev = -1isize;
    // let mut asm_items = Vec::new();
    let mut asm_items = Vec::with_capacity(state.selected_asm.len());

    for ins in state.cur_asm_range().map(|(_, v)| *v) {
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
            "{:<4} {:#010x}: {:<6} {:<30}",
            ins.serial_number,
            ins.address,
            ins.mnemonic,
            ins.op_str,
        ).to_string();

        asm_items.push(ListItem::new(vec![Spans::from(formatted_instruction)])
                .style(Style::default().fg(Color::Cyan)))

    }

    List::new(asm_items)
    .block(asm_block)
    // .highlight_style(Style::default()
    // .bg(Color::DarkGray)
    // .add_modifier(Modifier::BOLD))
}