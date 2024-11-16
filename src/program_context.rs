use std::sync::Arc;
use std::path::{Path,PathBuf};
use crate::file_parser::InstructionDetail;
use std::fs;
use crate::file_parser::Section;
use addr2line::Context;
use std::error::Error;
use std::collections::{HashMap,BTreeMap,HashSet};
use crate::file_parser::MachineFile;


pub type AddressFileMapping = HashMap<u64, (String, u32)>; // address -> (file, line)

pub fn map_instructions_to_source(
    machine_file: &MachineFile,
) -> Result<AddressFileMapping,Box<dyn Error>> {
    let mut mapping = AddressFileMapping::new();

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

pub struct Instruction{
    pub detail:InstructionDetail,
    pub file: Arc<PathBuf>
}

pub struct CodeFile {
    pub text: String,
    pub asm: BTreeMap<u32,Vec<Instruction>> //line -> instruction
}

impl CodeFile {
    pub fn read(path: &Path) -> Result<Self ,Box<dyn Error>>{
        let text = fs::read_to_string(path)?;
        Ok(CodeFile{
            text,
            asm: BTreeMap::new()
        }) 
    }
}

#[derive(Default)]
pub struct SourceMap {
    pub source_files :HashMap<PathBuf,CodeFile>,
    pub visited : HashSet<PathBuf>
}


impl SourceMap {
    pub fn new() -> Self {
        SourceMap::default()
    }

    pub fn visit_source_file(&mut self,path : &Path) -> Result<(),Box<dyn Error>>{
        if !self.visited.insert(path.to_path_buf()) {
            return Ok(());
        }

        let f = CodeFile::read(path)?;
        self.source_files.insert(path.to_path_buf(),f);
        Ok(())
    }

    pub fn visit_machine_file(&mut self,path : &Path) -> Result<(),Box<dyn Error>> {
        if !self.visited.insert(path.to_path_buf()) {
            return Ok(());
        }

        //read and parse the file
        let buffer = fs::read(path)?;
        let machine_file = MachineFile::parse(&buffer)?;
        let ctx = Context::from_dwarf(machine_file.dwarf_loader.load_dwarf()?)?;

        //add instructions to their spots

        let file_name = Arc::new(path.to_path_buf());

        for section in &machine_file.sections {
            match section {
                Section::Code(code_section) => {
                    for instruction in &code_section.instructions {
                        
                        //ignore missing but not invalid
                        if let Some(loc) = ctx.find_location(instruction.address)?{
                            if let (Some(file),Some(line)) = (loc.file,loc.line){
                                
                                //get the source file
                                let file = Path::new(file);
                                self.visit_source_file(file)?;
                                let file = self.source_files.get_mut(file).unwrap();

                                //insert
                                let x = Instruction {
                                    detail: instruction.clone(),
                                    file: file_name.clone()
                                };

                                file.asm.entry(line).or_default().push(x);

                            }
                        }
                    }
                },
                _ => todo!(),
            }
        } ;
        todo!()

    }
}