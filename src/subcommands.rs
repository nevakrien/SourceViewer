
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
        .expect("FILES argument is required") 
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
        .expect("FILES argument is required")
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
        .expect("FILES argument is required")
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

pub fn view_source_command(matches: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    // Collect all file paths provided by the user for the `view_source` command
    let file_paths: Vec<PathBuf> = matches
        .get_many::<PathBuf>("FILES")
        .expect("FILES argument is required")
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

    // Placeholder to simulate user selecting a file
    for file_name in source_files.iter() {
        let file :PathBuf= match fs::canonicalize(Path::new(file_name)) {
            Ok(file) =>file,
            Err(_) => {
                println!("{}",format!("{:?} does not exist",file_name ).red());
                continue;
            },
        };

        match std::fs::read_to_string(&file) {
            Ok(source_text) => {
                // Display the source text
                println!("Contents of {:?}:", file);
                for (i, line) in source_text.lines().enumerate() {
                    println!("{:4} {}", i + 1, line);
                }
            }
            Err(e) => {
                println!("{} reading {:?}: {}","FAILED".red(), file,e);

            }
        }

        
    }

    Ok(())
}
