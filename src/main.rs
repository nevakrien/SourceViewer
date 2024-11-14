use gimli::RunTimeEndian;
use std::error::Error;
use std::path::PathBuf;
use std::fs;
use source_viewer::file_parser::MachineFile;
use source_viewer::file_parser::Section;

use std::collections::HashMap;

use addr2line::Context; //seems to not work with windows

type FileLineMapping = HashMap<u64, (String, u32)>; // address -> (file, line)

pub fn map_instructions_to_source<'a>(
    machine_file: &MachineFile<'a>,
) -> Result<FileLineMapping,Box<dyn Error>> {
    let mut mapping = FileLineMapping::new();

    // Create addr2line context from DWARF data
    let ctx = Context::from_dwarf(machine_file.dwarf_loader.load_dwarf()?)?;

    // Iterate through each code section and map addresses to source
    for section in &machine_file.sections {
        if let Section::Code(code_section) = section {
            for instruction in &code_section.instructions {
                if let Ok(Some(loc)) = ctx.find_location(instruction.address) {
                    let file = loc.file.unwrap_or("<unknown>").to_string();
                    let line = loc.line.unwrap_or(0);
                    mapping.insert(instruction.address, (file, line));
                }
            }
        }
    }

    Ok(mapping)
}


fn annotate_instructions<'a>(machine_file: &MachineFile<'a>) -> Result<(),Box<dyn Error>>{
    let source_map = map_instructions_to_source(machine_file)?;

    for section in &machine_file.sections {
        if let Section::Code(code_section) = section {
            for instruction in &code_section.instructions {
                if let Some((file, line)) = source_map.get(&instruction.address) {
                    println!(
                        "Instruction at 0x{:x} in file {} at line {}",
                        instruction.address, file, line
                    );
                }
            }
        }
    }
    Ok(())
}



fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <ASM FILE>", args[0]);
        std::process::exit(1);
    }
    
    let file_path = PathBuf::from(&args[1]);
    let buffer = fs::read(&file_path)?;

    let parsed_executable = MachineFile::parse(&buffer)?;

    annotate_instructions(&parsed_executable)?;

    Ok(())
}
