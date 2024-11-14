use clap::{Arg, Command};
use colored::*;
use typed_arena::Arena;
use source_viewer::program_context::AsmRegistry;
use source_viewer::file_parser::Section;
use std::path::PathBuf;
use std::error::Error;
use source_viewer::file_parser::MachineFile;
use source_viewer::program_context::map_instructions_to_source;

fn annotate_instructions<'a>(machine_file: &MachineFile<'a>) -> Result<(), Box<dyn Error>> {
    let source_map = map_instructions_to_source(machine_file)?;

    for section in &machine_file.sections {
        if let Section::Code(code_section) = section {
            println!("{}",section.name());
            for (i,instruction) in code_section.instructions.iter().enumerate() {
                if let Some((file, line)) = source_map.get(&instruction.address) {
                    println!(
                        "{} \"{}\" {} {} {} {} {}",
                        i.to_string().blue(),
                        instruction.to_string().bold(),
                        "in file".cyan(),
                        file.to_string().yellow(),
                        "at line".cyan(),
                        line.to_string().blue(),
                        ""
                    );
                }
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    // Set up Clap to accept multiple files as arguments
    let matches = Command::new("Source Annotator")
        .version("1.0")
        .author("Your Name")
        .about("Annotates assembly instructions with source information")
        .arg(
            Arg::new("FILES")
                .help("Input assembly files to process")
                .required(true)
                .num_args(1..) // Allows multiple file paths
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .get_matches();

    // Collect all file paths provided by the user
    let file_paths: Vec<PathBuf> = matches
        .get_many::<PathBuf>("FILES")
        .unwrap() // We can unwrap because this is a required argument
        .cloned()
        .collect();

    // Create a new arena and registry to manage the files
    let arena = Arena::new();
    let mut registry = AsmRegistry::new(&arena);

    // Iterate over each file path and process it
    for file_path in file_paths {
        println!("{}", format!("Loading file {:?}", file_path).green().bold());
        registry.add_file(file_path.clone())?;
        let parsed_executable = registry.map.get(&file_path).ok_or("File not found in registry")?;
        
        // Annotate instructions for each file
        annotate_instructions(parsed_executable)?;
    }

    Ok(())
}
