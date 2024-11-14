use source_viewer::file_parser::Section;
use std::fs;
use std::path::PathBuf;
use std::error::Error;
use source_viewer::file_parser::MachineFile;
use source_viewer::program_context::map_instructions_to_source;


fn annotate_instructions<'a>(machine_file: &MachineFile<'a>) -> Result<(),Box<dyn Error>>{
    let source_map = map_instructions_to_source(machine_file)?;

    for section in &machine_file.sections {
        if let Section::Code(code_section) = section {
            for instruction in &code_section.instructions {
                if let Some((file, line)) = source_map.get(&instruction.address) {
                    println!(
                        "Instruction \"{}\"        in file {} at line {}",
                        instruction, file, line
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
