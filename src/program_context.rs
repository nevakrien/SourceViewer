use crate::errors::WrappedError;
use addr2line::LookupContinuation;
use addr2line::LookupResult;
use gimli::EndianSlice;
use gimli::RunTimeEndian;
use std::sync::Arc;
use std::path::{Path,PathBuf};
use crate::file_parser::InstructionDetail;
use std::fs;
use crate::file_parser::Section;
use addr2line::Context;
use std::error::Error;
use std::collections::{HashMap,BTreeMap,HashSet};
use crate::file_parser::MachineFile;


use typed_arena::Arena;
use  std::collections::hash_map;

//probably needed to handle the suplementry matrial


pub struct AsmRegistry<'a> {
    pub files_arena: &'a Arena<Vec<u8>>,
    pub map: HashMap<Arc<Path>, Result<MachineFile<'a>,Box<WrappedError>> >
}




impl<'a> AsmRegistry<'a> {
    pub fn new( files_arena: &'a Arena<Vec<u8>>) -> Self {
        AsmRegistry{
            files_arena,
            map:HashMap::new()
        }
    }



    pub fn get_machine(&mut self, path:Arc<Path>) -> Result<&MachineFile<'a>,Box<dyn Error>>{
        //code looks so ugly because we cant pull into a side function or the borrow checker will freak out
        // println!("geting data for {}",path.to_string_lossy());

        match self.map.entry(path.clone()) {
            hash_map::Entry::Occupied(entry) => entry.into_mut().as_ref().map_err(|e| e.clone().into()),
            hash_map::Entry::Vacant(entry) => {
                let buffer = match fs::read(&*path){
                    Ok(x) => x,
                    Err(e) => {return entry.insert(
                        Err(
                            Box::new(WrappedError::new(Box::new(e)))
                            ))
                        .as_ref().map_err(|e| e.clone().into())}
                };
                let b = self.files_arena.alloc(buffer);
                entry.insert(MachineFile::parse(b)
                    .map_err(|e| Box::new(WrappedError::new(e))))
                    .as_ref().map_err(|e| e.clone().into())
            }
        }
        
    }
}



pub type AddressFileMapping = HashMap<u64, (String, u32)>; // address -> (file, line)

pub fn map_instructions_to_source(
    machine_file: &MachineFile,
) -> Result<AddressFileMapping,Box<dyn Error>> {
    let mut mapping = AddressFileMapping::new();

    // Create addr2line context from DWARF data
    let ctx = Context::from_dwarf(machine_file.load_dwarf()?)?;

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

pub type DebugContext<'a> = addr2line::Context<EndianSlice<'a, RunTimeEndian>>;

pub fn resolve_func_name(addr2line: &DebugContext,address: u64) ->Option<String>{
    // Start the frame lookup process
        let lookup_result = addr2line.find_frames(address);

        let mut frames = lookup_result.skip_all_loads().ok()?;
        while let Ok(Some(frame)) = frames.next() {
            if let Some(name) = frame.function {
                return name.demangle().ok().map(|s| s.to_string());

            }
        }
        None 
}          
pub fn find_func_name<'a,'b:'a>(addr2line: &DebugContext<'a >, registry: &mut AsmRegistry<'b>, address: u64) -> Option<String> {  
    let mut lookup_result = addr2line.find_frames(address);

    loop {
        match lookup_result {
            LookupResult::Load { load, continuation } => {
                
                // println!("load case {:?} {:?}",load.parent,load.path);

                // Construct the full path for the DWO file if possible
               let dwo_path = load.comp_dir.as_ref()
                .map(|comp_dir| std::path::PathBuf::from(comp_dir.to_string_lossy().to_string()))
                .and_then(|comp_dir_path| load.path.as_ref()
                    .map(|path| comp_dir_path.join(std::path::Path::new(&path.to_string_lossy().to_string())))
                );

                // println!("load case {:?}",dwo_path);

                
                let dwo = dwo_path.and_then(|full_path| 
                    registry.get_machine(full_path.into()).ok()
                        .and_then(|m| m.load_dwarf().ok())
                        .map(Arc::new)
                );


                // Resume the lookup with the loaded data
                lookup_result = continuation.resume(dwo);
            }
            LookupResult::Output(Ok(mut frames)) => {
                // println!("existing case");

                while let Ok(Some(frame)) = frames.next() {
                    if let Some(name) = frame.function {
                        return name.demangle().ok().map(|s| s.to_string());
                    }
                }
                return None;
            }
            LookupResult::Output(Err(_e)) => {
                // println!("error case {}",e);

                return None;
            }
        }
    }
}



#[derive(PartialEq)]
pub struct Instruction{
    pub detail:InstructionDetail,
    pub file: Arc<Path>
}

#[derive(PartialEq)]
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
pub struct CodeRegistry {
    pub source_files :HashMap<PathBuf,CodeFile>,
    pub visited : HashSet<Arc<Path>>,
    // pub asm: AsmRegistry<'a>,
}


impl CodeRegistry {
    pub fn new() -> Self {
        CodeRegistry::default()
    }

    pub fn get_source_file(&mut self,path : &Path) -> Result<&CodeFile,Box<dyn Error>>{
        self.visit_source_file(path)?;
        self.source_files.get(path).ok_or("failed to retrive source file".into())
    }

    pub fn visit_source_file(&mut self,path : &Path) -> Result<(),Box<dyn Error>>{
        if !self.visited.insert(path.into()) {
            return Ok(());
        }

        let f = CodeFile::read(path)?;
        self.source_files.insert(path.to_path_buf(),f);
        Ok(())
    }

    pub fn visit_machine_file(&mut self,path : Arc<Path>,asm:&mut AsmRegistry) -> Result<(),Box<dyn Error>> {
        if !self.visited.insert(path.clone()) {
            return Ok(());
        }

        //read and parse the file
        let machine_file = asm.get_machine(path.clone())?;
        let ctx = Context::from_dwarf(machine_file.load_dwarf()?)?;


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
                                    file: path.clone()
                                };

                                file.asm.entry(line).or_default().push(x);

                            }
                        }
                    }
                },
                _ => {},
            }
        } ;
        // todo!()
        Ok(())

    }
}



pub struct DebugInstruction<'a>{
    ins: InstructionDetail,
    addr2line: &'a addr2line::Context<EndianSlice<'a, RunTimeEndian>>,
    //needs a way to load the Sup files which are machine files... 
    //probably means we need the asm registry
}

impl<'a> DebugInstruction<'a> {
    pub fn new(ins: InstructionDetail,addr2line: &'a addr2line::Context<EndianSlice<'a, RunTimeEndian>>) -> Self {
        DebugInstruction{ins,addr2line}
    }

    pub fn get_func_name(&self ) ->Option<String> {
        self.resolve_function_name(self.ins.address)
    }

    pub fn get_string_load<'b:'a>(&self, registry: &mut AsmRegistry<'b>) -> String {
        format!("{:#010x}: {:<6} {:<30} {}",
            
            self.ins.address, 
            self.ins.mnemonic,
            self.ins.op_str, //this needs a fixup
            find_func_name(self.addr2line,registry,self.ins.address).unwrap_or("<unknown>".to_string()),
        )
    }

    pub fn get_string_no_load(&self) -> String {
        format!("{:#010x}: {:<6} {:<30} {}",
            
            self.ins.address, 
            self.ins.mnemonic,
            self.ins.op_str, //this needs a fixup
            self.get_func_name().unwrap_or("<unknown>".to_string()),
        )
    }

    /// Resolve the function name for a given address using addr2line
    fn resolve_function_name(&self, address: u64) -> Option<String> {
        resolve_func_name(self.addr2line,address)
    }
}