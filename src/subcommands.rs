use crate::file_parser::DebugInstruction;
use crate::file_parser::MachineFile;
use std::fs;
use std::collections::HashSet;
use crate::program_context::AddressFileMapping;
use colored::*;
use crate::file_parser::{Section};
use std::path::PathBuf;
use std::error::Error;
use crate::program_context::map_instructions_to_source;

pub fn lines_command(matches: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    // Collect all file paths provided by the user for the `lines` command
    let file_paths: Vec<PathBuf> = matches
        .get_many::<PathBuf>("FILES")
        .expect("FILES argument is required") 
        .cloned()
        .collect();


    // Iterate over each file path and process it
    for file_path in file_paths {
        println!("{}", format!("Loading file {:?}", file_path).green().bold());
        let buffer = fs::read(file_path)?;
        let machine_file = MachineFile::parse(&buffer)?;
        let ctx = addr2line::Context::from_dwarf(machine_file.dwarf_loader.load_dwarf()?)?;

        let source_map = map_instructions_to_source(&machine_file)?;

        for section in &machine_file.sections {
            if let Section::Code(code_section) = section {
                println!("{}", section.name());
                for (i, instruction) in code_section.instructions.iter().enumerate() {
                    if let Some((file, line)) = source_map.get(&instruction.address) {
                        let debug_ins = DebugInstruction::new(instruction.clone(),&ctx);

                        println!(
                            "{} \"{}\" {} {} {} {} ",
                            i.to_string().blue(),
                            debug_ins.to_string().bold(),
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

pub fn sections_commands(matches: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
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
        let machine_file = MachineFile::parse(&buffer)?;
        
        for section in &machine_file.sections {
            match section {
                Section::Code(code_section) => {
                    println!(
                        "Code Section: {} ({} instructions)",
                        code_section.name,
                        code_section.instructions.len()
                    );
                    for instruction in &code_section.instructions {
                        println!("  {}", instruction);
                    }
                }
                Section::Info(non_exec_section) => {
                    println!(
                        "Non-Executable Section: {} ({} bytes)",
                        non_exec_section.name,
                        non_exec_section.data.len()
                    );
                }
            }
        }
    }

    Ok(())
}

pub fn source_view_command(matches: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    // Collect all file paths provided by the user for the `source_view` command
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
        let machine_file = MachineFile::parse(&buffer)?;

        let map = map_instructions_to_source(&machine_file)?;
        for (s,_) in map.values() {
            source_files.insert(s.to_string());
        }
        filemaps.push(map);
    }



    println!("Select a file to view:");
    for (index, file) in source_files.iter().enumerate() {
        println!("{}: {:?}", index, file);
    }

    // Placeholder to simulate user selecting a file
    for file in source_files.iter() {
        let source_text = std::fs::read_to_string(file)?;

        // Display the source text
        println!("Contents of {:?}:", file);
        for (i, line) in source_text.lines().enumerate() {
            println!("{:4} {}", i + 1, line);
        }
    }

    Ok(())
}
