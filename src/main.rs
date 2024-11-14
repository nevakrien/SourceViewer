// use gimli::Reader;
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

// pub fn map_instructions_to_source<'a>(
//     machine_file: &MachineFile<'a>,
// ) -> Result<FileLineMapping, Box<dyn Error>> {
//     let mut mapping = FileLineMapping::new();

//     // Load the DWARF data directly from the provided loader
//     let dwarf = machine_file.dwarf_loader.load_dwarf()?;

//     // Iterate through each unit in the DWARF data
//     let mut units = dwarf.units();
//     while let Some(header) = units.next()? {
//         let unit = dwarf.unit(header)?;

//         // Check if the line program exists; if not, return an error
//         let program = unit
//             .line_program
//             .as_ref()
//             .ok_or_else(|| "Missing line program in DWARF unit")?;
        
//         // Set up rows iterator to access file and line mappings
//         let mut rows = program.clone().rows();
//         while let Ok(Some((header, row))) = rows.next_row() {
//             let address = row.address();

//             // Retrieve the file entry and handle missing cases by returning an error
//             let file_entry = row
//                 .file(header)
//                 .ok_or_else(|| "Missing file entry in line row")?;

//             // Convert the path from `AttributeValue` to a String
//             let file = match file_entry.path_name() {
//                 AttributeValue::DebugStrRef(offset) => {
//                     dwarf.debug_str.get_str(offset)?.to_string_lossy().into_owned()
//                 },
//                 AttributeValue::DebugLineStrRef(offset) => {
//                     dwarf.debug_line_str.get_str(offset)?.to_string_lossy().into_owned()
//                 },
//                 AttributeValue::String(slice) => {
//                     String::from_utf8(slice.to_slice()?.to_vec())?
//                 },
//                 other => {
//                     // Print the unexpected `AttributeValue` before returning an error
//                     // eprintln!("Unexpected attribute value for file path: {:?}", other);
//                     return Err(format!("Unexpected attribute value for file path: {:?}", other).into());
//                 }
//             };

//             // Convert NonZeroU64 line to u32, or return an error if line is missing
//             let line = row.line().ok_or_else(|| "Missing line number")?.get() as u32;

//             mapping.insert(address, (file, line));
//         }
//     }

//     // Map instructions to source files and lines
//     for section in &machine_file.sections {
//         if let Section::Code(code_section) = section {
//             for instruction in &code_section.instructions {
//                 if let Some((file, line)) = mapping.get(&instruction.address) {
//                     mapping.insert(instruction.address, (file.clone(), *line));
//                 }
//             }
//         }
//     }

//     Ok(mapping)
// }



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
