use std::path::PathBuf;
use std::fs;
use crate::file_parser::Section;
use addr2line::Context;
use std::error::Error;
use std::collections::HashMap;
use crate::file_parser::MachineFile;

use typed_arena::Arena;

pub struct AsmRegistry<'a> {
    pub files_areana: &'a Arena<Vec<u8>>,
    pub map: HashMap<PathBuf, MachineFile<'a>>
}

impl<'a> AsmRegistry<'a> {
    pub fn new( files_areana: &'a Arena<Vec<u8>>) -> Self {
        AsmRegistry{
            files_areana,
            map:HashMap::new()
        }
    }
    pub fn add_file(&mut self, path:PathBuf) -> Result<(),Box<dyn Error>>{
        let buffer = fs::read(&path)?;
        let b = self.files_areana.alloc(buffer);
        

        let m = MachineFile::parse(b)?;
        self.map.insert(path,m);
        Ok(())
    }
}

pub type FileLineMapping = HashMap<u64, (String, u32)>; // address -> (file, line)

pub fn map_instructions_to_source(
    machine_file: &MachineFile,
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

