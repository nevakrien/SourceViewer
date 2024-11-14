use std::error::Error;
use std::path::PathBuf;
use std::fs;
use source_viewer::file_parser::MachineFile;
use source_viewer::file_parser::Section;


fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <ASM FILE>", args[0]);
        std::process::exit(1);
    }
    
    let file_path = PathBuf::from(&args[1]);
    let buffer = fs::read(&file_path)?;

    let parsed_executable = MachineFile::parse(&buffer)?;

    println!("Parsed file: {}", file_path.display());
    for section in &parsed_executable.sections {
        match section {
            Section::Code(code_section) => {
                println!("Code Section: {} ({} instructions)", code_section.name, code_section.instructions.len());
                for instruction in &code_section.instructions {
                    println!("  {}", instruction);
                }
            }
            Section::Info(non_exec_section) => {
                println!("Non-Executable Section: {} ({} bytes)", non_exec_section.name, non_exec_section.data.len());
            }
        }
    }
    println!("Loading DWARF");
    println!("{:?}", parsed_executable.dwarf_loader.load_dwarf());

    Ok(())
}
