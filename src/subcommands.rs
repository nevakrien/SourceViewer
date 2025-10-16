use fallible_iterator::FallibleIterator;
use crate::args::FileSelection;
use crate::program_context::find_func_name;
use crate::program_context::CodeRegistry;
use crate::walk::FileResult;
use crate::walk::GlobalState;
use crate::walk::TerminalSession;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::file_parser::MachineFile;
use crate::file_parser::Section;
use crate::program_context::resolve_func_name;
use crate::program_context::FileRegistry;
// use crate::program_context::format_inst_debug;
use colored::*;
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use typed_arena::Arena;

use crate::println;

// use crate::program_context::AddressFileMapping;

pub fn walk_command(obj_file: Arc<Path>) -> Result<(), Box<dyn std::error::Error>> {
    let asm_arena = Arena::new();
    let code_arena = Arena::new();
    let mut registry = FileRegistry::new(&asm_arena);
    let mut code_files = CodeRegistry::new(&mut registry, &code_arena);

    println!("visiting file {:?}", &*obj_file);
    code_files
        .visit_machine_file(obj_file.clone())?
        .get_lines_map()?;

    // let mut terminal = create_terminal()?;
    // let _cleanup = TerminalCleanup;
    let mut state = GlobalState::start()?;
    let mut session = TerminalSession::new(&mut state)?;

    session.walk_directory_loop(&mut code_files, obj_file)
}

pub fn lines_command(file_paths: Vec<PathBuf>, ignore_unknown: bool) -> Result<(), Box<dyn Error>> {
    let arena = Arena::new();
    let mut registry = FileRegistry::new(&arena);
    // Iterate over each file path and process it
    for file_path in file_paths {
        println!("{}", format!("Loading file {:?}", file_path).green().bold());
        let machine_file = registry.get_machine(file_path.into())?;
        let arch = machine_file.obj.architecture();
        let ctx = machine_file.get_addr2line()?;

        for section in &machine_file.sections.clone() {
            if let Section::Code(code_section) = section {
                println!("{}", section.name());

                for (_i, ins) in code_section.get_asm(arch)?.iter().enumerate() {
                    let (file, line) = match ctx.find_location(ins.address)? {
                        Some(loc) => {
                            if ignore_unknown && (loc.file.is_none()||loc.line.is_none()){
                                continue;
                            }
                            let file = loc.file.unwrap_or("<unknown>").to_string();
                            let line = loc.line.map(|i| {i.to_string()}).unwrap_or("<unknown>".to_string());
                            (file, line)
                        },
                        None => {
                            if ignore_unknown {
                                continue;
                            }
                            ("<unknown>".to_string(), "<unknown>".to_string())
                        }
                    };
                    let asm = format!(
                        "{:#010x}: {:<6} {:<15}",
                        ins.address,
                        ins.mnemonic,
                        ins.op_str, //this needs a fixup
                    );

                    let func = find_func_name(&ctx, &mut registry, ins.address)
                        .unwrap_or("<unknown>".to_string());

                    println!(
                        "{} {} {}:{}",
                        asm.bold(),
                        func.cyan(),
                        file.to_string().yellow(),
                        line.to_string().blue()
                    );
                }
            }
        }
    }

    Ok(())
}

use object::{File, Object, ObjectSection};
fn list_dwarf_sections<'a>(obj_file: &'a File<'a>) -> Result<(), Box<dyn std::error::Error>> {
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
        let section_data = obj_file
            .section_by_name(section_name)
            .and_then(|x| x.data().ok())
            .unwrap_or(&[]);

        // Print the section name and content as UTF-8 (if possible)
        println!(
            "{}:\n{}",
            section_name.blue(),
            String::from_utf8_lossy(section_data)
        );
    }
    Ok(())
}

pub fn dwarf_dump_command(file_paths: Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    let message = "NOTE: this comand is not finised".to_string().red();
    println!("{}", message);
    // Iterate over each file path and process it
    for file_path in file_paths {
        println!("{}", format!("Loading file {:?}", file_path).green().bold());
        let buffer = fs::read(file_path)?;
        let machine_file = MachineFile::parse(&buffer, false)?;
        // let dwarf = machine_file.load_dwarf()?;
        // println!("{:#?}",dwarf );
        list_dwarf_sections(&machine_file.obj)?;
    }
    println!("{}", message);

    Ok(())
}

pub fn sections_command(file_paths: Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    // Iterate over each file path and process it
    for file_path in file_paths {
        println!("{}", format!("Loading file {:?}", file_path).green().bold());
        let buffer = fs::read(file_path)?;
        let mut machine_file = MachineFile::parse(&buffer, true)?;
        let debug = machine_file.get_addr2line().ok();

        for section in &mut machine_file.sections {
            match section {
                Section::Code(code_section) => {
                    // lazy.disasm(&machine_file.obj.architecture())?;
                    println!(
                        "Code Section: {} ({} instructions)",
                        code_section.name.blue(),
                        code_section.get_existing_asm().len()
                    );

                    for instruction in code_section.get_existing_asm().iter() {
                        let func_name = match &debug {
                            None => None,
                            Some(ctx) => resolve_func_name(ctx, instruction.address),
                        };
                        // func_name.as_mut().map(|x| x.push_str(" "));
                        // println!("  {}", instruction);
                        println!(
                            "  {:#010x}: {:<6} {:<30} {}",
                            instruction.address,
                            instruction.mnemonic,
                            instruction.op_str,
                            func_name.as_deref().unwrap_or("")
                        )
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


pub fn view_sources_command(file_paths: Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    let mut source_files: HashSet<Box<str>> = HashSet::new();
    for file_path in file_paths {
        println!("{}", format!("Loading file {:?}", file_path).green().bold());
        let buffer = fs::read(file_path)?;
        let machine_file = MachineFile::parse(&buffer, true)?;
        let ctx = machine_file.get_addr2line()?;
        for section in machine_file.sections.iter(){
            let Section::Code(code) = section else{
                continue;
            };

            let mut locs = ctx.find_location_range(code.address,code.address+code.data.len() as u64)?;
            while let Some((_,_,loc)) =FallibleIterator::next(&mut locs)?{
                if let Some(file) = loc.file{
                    source_files.insert(file.into());
                }
            }

        }
    }


    let mut source_files: Vec<_> = source_files.into_iter().collect();
    source_files.sort();

    println!("Source files:");
    for (index, file) in source_files.iter().enumerate() {
        println!("{}: {:?}", index, file);
    }
    Ok(())
}

pub fn view_source_command(
    file_path: &Path,
    look_all: bool,
    walk: bool,
    selections: Vec<FileSelection>,
) -> Result<(), Box<dyn Error>> {
    //we allow look_all and selections at the same time we simply ignore selctions

    //removing this just for dev
    // if walk && (look_all || selections.len() > 1) {
    //     return Err("Can only walk in 1 file at a time".into());
    // }

    if walk && selections.len() == 0 {
        return Err("No walk selection provided".into());
    }

    // Load and parse the binary
    let buffer = fs::read(file_path)?;
    let machine_file = MachineFile::parse(&buffer, true)?;
    let ctx = machine_file.get_addr2line()?;

    // Populate a unique list of source files in the order they appear
    let mut source_files_set: HashSet<PathBuf> = HashSet::new();
    for section in machine_file.sections.iter(){
        let Section::Code(code) = section else{
            continue;
        };

        let mut locs = ctx.find_location_range(code.address,code.address+code.data.len() as u64)?;
        while let Some((_,_,loc)) =FallibleIterator::next(&mut locs)?{
            if let Some(file) = loc.file{
                source_files_set.insert(file.into());
            }
        }

    }

    let mut source_files: Vec<&Path> = source_files_set.iter().map(|p| p.as_path()).collect();
    source_files.sort();

    if walk {
        let obj_file: Arc<Path> = file_path.into();
        let asm_arena = Arena::new();
        let code_arena = Arena::new();
        let mut registry = FileRegistry::new(&asm_arena);
        let mut code_files = CodeRegistry::new(&mut registry, &code_arena);
        code_files
            .visit_machine_file(obj_file.clone())?
            .get_lines_map()?;

        let file_path = match &selections[0] {
            FileSelection::Index(i) => {
                if let Some(file) = source_files.get(*i) {
                    *file
                } else {
                    println!("{}", format!("Index {} is out of bounds", i).red());
                    return Ok(());
                }
            }
            FileSelection::Path(path) => {
                if let Some(ans) = source_files_set.get(path) {
                    ans
                } else {
                    println!(
                        "{}",
                        format!("Path {:?} is not included in the binary", path).red()
                    );
                    return Ok(());
                }
            }
        };
        let file_path = Path::new(file_path.into());
        let parent = file_path
            .parent()
            .ok_or("No parent dir to path")?
            .to_path_buf();

        let mut state = GlobalState::start_from(parent)?;
        let mut session = TerminalSession::new(&mut state)?;

        //file
        {
            let mut file_state = crate::walk::load_file(session.state, file_path)?;
            if let Some(FileSelection::Index(i)) = selections.get(1) {
                file_state.file_scroll = i.saturating_sub(1);
                file_state.cursor = i.saturating_sub(1);
            };
            let code_file = code_files.get_source_file(file_path.into(),true)?;
            let mut last_frame = Instant::now();
            let res = TerminalSession::walk_file_loop(
                &mut last_frame,
                &mut session.terminal,
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

        return session.walk_directory_loop(&mut code_files, obj_file);
    }

    // Display source files with their indices
    println!("Source files:");
    for (index, file) in source_files.iter().enumerate() {
        println!("{}: {:?}", index, file);
    }

    // Collect files to display based on the selections or `-a` flag
    let mut files_to_display: Vec<&Path> = Vec::new();

    if look_all {
        // Add all files if `-a` is set
        files_to_display.extend(source_files.iter());
    } else {
        // Add files based on selections
        for selection in selections {
            match selection {
                FileSelection::Index(i) => {
                    if let Some(file) = source_files.get(i) {
                        files_to_display.push(file);
                    } else {
                        println!("{}", format!("Index {} is out of bounds", i).red());
                    }
                }
                FileSelection::Path(path) => {
                    if let Some(file) = source_files_set.get(&path) {
                        files_to_display.push(file);
                    } else {
                        println!(
                            "{}",
                            format!("Path {:?} is not included in the binary", path).red()
                        );
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
fn display_file_contents(file_path: &Path) -> Result<(), Box<dyn Error>> {
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
            println!("{}", format!("{:?} does not exist", file_path).red());
        }
    }
    Ok(())
}
