
use std::collections::HashMap;
use std::sync::Arc;
use std::path::Path;
use crate::program_context::CodeRegistry;
use crate::walk::render_file_asm_viewer;
use crate::walk::handle_file_input;
use crate::walk::create_terminal;
use crate::walk::TerminalCleanup;
use crate::walk::State;
use crate::walk::render_directory;
use crate::walk::handle_directory_input;
use crate::walk::Mode;


use crate::program_context::AsmRegistry;
use typed_arena::Arena;
use crate::program_context::resolve_func_name;
use crate::program_context::DebugInstruction;
use crate::file_parser::MachineFile;
use std::fs;
use std::collections::HashSet;
use crate::program_context::AddressFileMapping;
use colored::*;
use crate::file_parser::{Section};
use std::path::PathBuf;
use std::error::Error;
use crate::program_context::map_instructions_to_source;

pub fn walk_command(matches: &clap::ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let file_path: PathBuf = matches
    .get_one::<PathBuf>("FILE") // Use `get_one` instead of `get_many`
    .expect("FILE argument is required")
    .into(); // No need for `collect`, just convert directly to `PathBuf`
    let obj_file :Arc<Path>= file_path.into();

    let arena = Arena::new();
    let mut registry = AsmRegistry::new(&arena);
    let mut code_files = CodeRegistry::new(&mut registry);
    
    println!("visiting file {:?}",&*obj_file);
    code_files.visit_machine_file(obj_file.clone())?
    .get_lines_map()?;


    let mut terminal = create_terminal()?;
    let _cleanup = TerminalCleanup;
    let mut state = State::new();

    state.dir_list_state.select(Some(0)); // Initialize the selected index

    loop {
        match state.mode {
            Mode::Dir => {
                let entries: Vec<_> = fs::read_dir(&state.current_dir)?
                    .filter_map(Result::ok)
                    .collect();
                render_directory(&mut terminal, &entries, &mut state)?;
                if handle_directory_input(&entries, &mut state)? {
                    break;
                }
            }
            Mode::File => {
                let path =  fs::canonicalize(Path::new(&state.file_path))?.into();
                let file = code_files.get_source_file(path)?;

                render_file_asm_viewer(&mut terminal, &mut state,file,obj_file.clone())?;
                if handle_file_input(&mut state)? {
                    break;
                }
            }
        };
    }

    Ok(())
}


pub fn lines_command(matches: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    // Collect all file paths provided by the user for the `lines` command
    let file_paths: Vec<PathBuf> = matches
        .get_many::<PathBuf>("FILES")
        .ok_or("FILES argument is required")? 
        .cloned()
        .collect();

    let arena = Arena::new();
    let mut registry = AsmRegistry::new(&arena);
    // Iterate over each file path and process it
    for file_path in file_paths {
        println!("{}", format!("Loading file {:?}", file_path).green().bold());
        let machine_file = registry.get_machine(file_path.into())?;
        let ctx = addr2line::Context::from_arc_dwarf(machine_file.load_dwarf()?)?;

        let source_map = map_instructions_to_source(machine_file)?;

        for section in &machine_file.sections.clone() {
            if let Section::Code(code_section) = section {
                println!("{}", section.name());
                for (i, instruction) in code_section.instructions.iter().enumerate() {
                    if let Some((file, line)) = source_map.get(&instruction.address) {
                        let debug_ins = DebugInstruction::new(instruction.clone(),&ctx);

                        println!(
                            "{:<4} {} {} {} {} {} ",
                            i.to_string().blue(),
                            debug_ins.get_string_load(&mut registry).bold(),
                            "in file".cyan(),
                            file.to_string().yellow(),
                            "at line".cyan(),
                            line.to_string().blue()
                        );
                    }
                }
            }
        }
    }

    Ok(())
}


use object::{File, Object,ObjectSection};
fn list_dwarf_sections<'a>(obj_file: &'a File<'a>) {
    let sections = [
        ".debug_abbrev",
        ".debug_addr",
        ".debug_aranges",
        ".debug_info",
        ".debug_line",
        ".debug_line_str",
        ".debug_str",
        ".debug_str_offsets",
        ".debug_types",
        ".debug_loc",
        ".debug_ranges",
    ];

    for section_name in &sections {
        // Find the section by name, get the data if available, or return an empty slice
        let section_data = obj_file.section_by_name(section_name).and_then(|x| x.data().ok()).unwrap_or(&[]);
        
        // Print the section name and content as UTF-8 (if possible)
        println!("{}:\n{}", section_name.blue(), String::from_utf8_lossy(section_data));
    }
}

pub fn dwarf_dump_command(matches: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    // Collect all file paths provided by the user for the `sections` command
    let file_paths: Vec<PathBuf> = matches
        .get_many::<PathBuf>("FILES")
        .ok_or("FILES argument is required")?
        .cloned()
        .collect();

    let message = "NOTE: this comand is not finised".to_string().red();
    println!("{}", message);
    // Iterate over each file path and process it
    for file_path in file_paths {
        println!("{}", format!("Loading file {:?}", file_path).green().bold());
        let buffer = fs::read(file_path)?;
        let machine_file = MachineFile::parse(&buffer)?;
        // let dwarf = machine_file.load_dwarf()?;
        // println!("{:#?}",dwarf );
        list_dwarf_sections(&machine_file.obj);
        
    }
    println!("{}", message);

    Ok(())
}

pub fn sections_command(matches: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    // Collect all file paths provided by the user for the `sections` command
    let file_paths: Vec<PathBuf> = matches
        .get_many::<PathBuf>("FILES")
        .ok_or("FILES argument is required")?
        .cloned()
        .collect();


    // Iterate over each file path and process it
    for file_path in file_paths {
        println!("{}", format!("Loading file {:?}", file_path).green().bold());
        let buffer = fs::read(file_path)?;
        let mut machine_file = MachineFile::parse(&buffer)?;
        let debug = machine_file.load_dwarf().ok().and_then(|dwarf_data| {
            addr2line::Context::from_arc_dwarf(dwarf_data).ok()
        });

        
        for section in &machine_file.sections {
            match section {
                Section::Code(code_section) => {
                    println!(
                        "Code Section: {} ({} instructions)",
                        code_section.name.blue(),
                        code_section.instructions.len()
                    );

                    

                    for instruction in &code_section.instructions {
                        let func_name = match &debug {
                            None => None,
                            Some(ctx) => resolve_func_name(ctx,instruction.address)

                        };
                        // func_name.as_mut().map(|x| x.push_str(" "));
                        // println!("  {}", instruction);
                        println!("  {:#010x}: {:<6} {:<30} {}",instruction.address, instruction.mnemonic, instruction.op_str
                            ,func_name.as_deref().unwrap_or(""))
                    }
                }
                Section::Info(non_exec) => {
                    println!(
                        "Non-Executable Section: {} ({} bytes)",
                        non_exec.name.blue(),
                        non_exec.data.len()
                    );

                    // println!("{}", String::from_utf8_lossy(non_exec.data) );
                }
            }
        }
    }

    Ok(())
}

pub fn view_sources_command(matches: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    // Collect all file paths provided by the user for the `view_source` command
    let file_paths: Vec<PathBuf> = matches
        .get_many::<PathBuf>("BINS")
        .expect("BINS argument is required")
        .cloned()
        .collect();


    // Initialize a basic editor interface
    // TODO: Use a library like `crossterm` to set up the interface
    // For now, placeholder logic to prompt file selection
    let mut filemaps: Vec<AddressFileMapping> = Vec::new();
    let mut source_files: HashSet<String> = HashSet::new();


    // Load files into registry
    for file_path in file_paths {
        println!("{}", format!("Loading file {:?}", file_path).green().bold());
        // registry.add_file(file_path.clone())?;

        let buffer = fs::read(file_path)?;
        let mut machine_file = MachineFile::parse(&buffer)?;

        let map = map_instructions_to_source(&mut machine_file)?;
        for (s,_) in map.values() {
            source_files.insert(s.to_string());
        }
        filemaps.push(map);
    }



    println!("Source files:");
    for (index, file) in source_files.iter().enumerate() {
        println!("{}: {:?}", index, file);
    }
    Ok(())
}

#[derive(Debug,Clone)]
pub enum FileSelection {
    Index(usize),
    Path(PathBuf),
}





pub fn view_source_command(matches: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    // Collect the binary path from the `BIN` argument
    let file_path = matches.get_one::<PathBuf>("BIN").ok_or("BIN argument is required")?;

    // Check if the `-a` flag is set
    let set_flag = matches.get_flag("all");

    // Gather selections (either indices or paths)
    let selections: Vec<_> = matches.get_many::<FileSelection>("SELECTIONS").unwrap_or_default().collect();

    // Load and parse the binary
    let buffer = fs::read(file_path)?;
    let mut machine_file = MachineFile::parse(&buffer)?;
    let map = map_instructions_to_source(&mut machine_file)?;

    // Populate a unique list of source files in the order they appear
    let mut source_files: Vec<String> = Vec::new();
    let mut source_files_map: HashMap<String, usize> = HashMap::new();

    for (source, _) in map.values() {
        // Add to source files if not already added
        if !source_files_map.contains_key(source) {
            let index = source_files.len();
            source_files.push(source.to_string());
            source_files_map.insert(source.to_string(), index);
        }
    }

    // Display source files with their indices
    println!("Source files:");
    for (index, file) in source_files.iter().enumerate() {
        println!("{}: {:?}", index, file);
    }

    // Collect files to display based on the selections or `-a` flag
    let mut files_to_display: Vec<&String> = Vec::new();

    if set_flag {
        // Add all files if `-a` is set
        files_to_display.extend(source_files.iter());
    } else {
        // Add files based on selections
        for selection in selections.iter() {
            match selection {
                FileSelection::Index(i) => {
                    if let Some(file) = source_files.get(*i) {
                        files_to_display.push(file);
                    } else {
                        println!("{}", format!("Index {} is out of bounds", i).red());
                    }
                }
                FileSelection::Path(path) => {
                    let path_str = path.to_string_lossy().to_string();
                    if let Some(&index) = source_files_map.get(&path_str) {
                        files_to_display.push(&source_files[index]);
                    } else {
                        println!("{}", format!("Path {:?} is not included in the binary", path).red());
                    }
                }
            }
        }
    }

    // Display the contents of each file in `files_to_display`
    for file in files_to_display {
        display_file_contents(file)?;
    }

    Ok(())
}


// Helper function to display the contents of a file with line numbers
fn display_file_contents(file_name: &str) -> Result<(), Box<dyn Error>> {
    let file_path = Path::new(file_name);
    match fs::canonicalize(file_path) {
        Ok(file) => match fs::read_to_string(&file) {
            Ok(source_text) => {
                println!("Contents of {:?}:", file);
                for (i, line) in source_text.lines().enumerate() {
                    println!("{:4} {}", i + 1, line);
                }
            }
            Err(e) => {
                println!("{} reading {:?}: {}", "FAILED".red(), file, e);
            }
        },
        Err(_) => {
            println!("{}", format!("{:?} does not exist", file_name).red());
        }
    }
    Ok(())
}