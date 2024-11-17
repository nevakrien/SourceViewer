use crate::errors::StackedError;
use crate::errors::WrapedError;
use addr2line::LookupContinuation;
use addr2line::LookupResult;
use gimli::EndianSlice;
use gimli::RunTimeEndian;
use std::sync::Arc;
use std::path::{Path};
use crate::file_parser::InstructionDetail;
use std::fs;
use crate::file_parser::Section;
use addr2line::Context;
use std::error::Error;
use std::collections::{HashMap,BTreeMap};
use crate::file_parser::MachineFile;


use typed_arena::Arena;
use  std::collections::hash_map;

//probably needed to handle the suplementry matrial


pub struct AsmRegistry<'a> {
    pub files_arena: &'a Arena<Vec<u8>>,
    pub map: HashMap<Arc<Path>, Result<MachineFile<'a>, WrapedError> >
}




impl<'a> AsmRegistry<'a> {
    pub fn new( files_arena: &'a Arena<Vec<u8>>) -> Self {
        AsmRegistry{
            files_arena,
            map:HashMap::new()
        }
    }



    pub fn get_machine(&mut self, path:Arc<Path>) -> Result<&mut MachineFile<'a>,Box<dyn Error>>{
        //code looks so ugly because we cant pull into a side function or the borrow checker will freak out
        // println!("geting data for {}",path.to_string_lossy());

        match self.map.entry(path.clone()) {
            hash_map::Entry::Occupied(entry) => entry.into_mut().as_mut().map_err(|e| e.clone().into()),
            hash_map::Entry::Vacant(entry) => {
                let buffer = match fs::read(&*path){
                    Ok(x) => x,
                    Err(e) => {return entry.insert(
                        Err(
                            WrapedError::new(Box::new(e))
                            ))
                        .as_mut().map_err(|e| e.clone().into())}
                };
                let b = self.files_arena.alloc(buffer);
                entry.insert(MachineFile::parse(b)
                    .map_err(|e| WrapedError::new(e)))
                    .as_mut().map_err(|e| e.clone().into())
            }
        }
        
    }
}



pub type AddressFileMapping = HashMap<u64, (String, u32)>; // address -> (file, line)

pub fn map_instructions_to_source(
    machine_file: &mut MachineFile,
) -> Result<AddressFileMapping,Box<dyn Error>> {
    let mut mapping = AddressFileMapping::new();

    // Create addr2line context from DWARF data
    let ctx = Context::from_arc_dwarf(machine_file.load_dwarf()?)?;

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
                        // .map(Arc::new)
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



// #[derive(PartialEq,Clone)]
// pub struct Instruction{
//     pub detail:InstructionDetail,
//     pub file: Arc<Path>
// }

// #[derive(PartialEq)]
pub struct CodeFile {
    pub text: String,
    asm: BTreeMap<u32,HashMap<Arc<Path>,Vec<InstructionDetail>>>, //line -> instruction
    pub errors: Vec<(StackedError,Option<Arc<Path>>)>
}

impl CodeFile {
    pub fn read(path: &Path) -> Result<Self ,Box<dyn Error>>{
        let text = fs::read_to_string(path)?;
        Ok(CodeFile{
            text,
            asm: BTreeMap::new(),
            errors: Vec::new(),
        }) 
    }

    #[inline]
    pub fn get_asm(&self,line:&u32,path:Arc<Path>) -> Option<&[InstructionDetail]> {
        self.asm.get(&line)?.get(&path).map(|x| x.as_slice())//.unwrap_or(&[])
    }
}

pub struct CodeRegistry<'a,'b> {
    pub source_files :HashMap<Arc<Path>,Result<CodeFile,Box<WrapedError>>>,
    asm:&'b mut AsmRegistry<'a>
    // pub visited : HashSet<Arc<Path>>,
    // pub asm: AsmRegistry<'a>,
}


pub fn make_context<'a>(machine_file:&mut MachineFile<'a>) -> Result<Context<EndianSlice<'a,RunTimeEndian>>,Box<dyn Error>> {
    Ok(Context::from_arc_dwarf(machine_file.load_dwarf()?)?)
}

impl<'a,'b> CodeRegistry<'a,'b> {
    pub fn new(asm:&'b mut AsmRegistry<'a>) -> Self {
        CodeRegistry {
            asm,
            source_files : HashMap::new(),
        }
    }

    pub fn get_source_file(&mut self,path : Arc<Path>) -> Result<&mut CodeFile,Box<dyn Error>>{
        match self.source_files.entry(path.clone()) {
            hash_map::Entry::Occupied(entry)=> entry.into_mut().as_mut().map_err(|e| e.clone().into()),
            hash_map::Entry::Vacant(entry) => {
                let code_file = entry.insert(CodeFile::read(&path)
                    .map_err(|e| Box::new(WrapedError::new(e)))
                );

               let code_file = match code_file {
                    Ok(x) => x,
                    Err(e) => {return Err((*e).clone().into());}
               };

                for (obj_path,res) in self.asm.map.iter_mut() {
                    let machine_file = match res {
                        Ok(x) => x,
                        Err(e) => {
                            let error = StackedError::from_wraped(e.clone(),"while getting machine");
                            code_file.errors.push((error,None));
                            continue;
                        }
                    };
                    let map = match machine_file.get_lines_map() {
                        Ok(x) =>x,
                        Err(e) => {
                            let error = StackedError::new(e,"while making context");
                            code_file.errors.push((error,None));
                            continue;
                            
                        }
                    };

                    if let Some(line_map) = map.get(&path){
                        for (line,v) in line_map {
                            let spot = code_file.asm
                            .entry(*line)
                            .or_insert(HashMap::new())
                            .entry(obj_path.clone())
                            .or_insert(vec![]);

                            spot.reserve(v.len());
                            spot.extend_from_slice(v)
                        }
                    }

                    
                }

                
                Ok(code_file)
            }
        }
    }


    // pub fn get_source_file(&mut self,path : Arc<Path>) -> Result<&mut CodeFile,Box<dyn Error>>{
    //     match self.source_files.entry(path.clone()) {
    //         hash_map::Entry::Occupied(entry)=> entry.into_mut().as_mut().map_err(|e| e.clone().into()),
    //         hash_map::Entry::Vacant(entry) => {
    //             let code_file = entry.insert(CodeFile::read(&path)
    //                 .map_err(|e| Box::new(WrapedError::new(e)))
    //             );

    //            let code_file = match code_file {
    //                 Ok(x) => x,
    //                 Err(e) => {return Err((*e).clone().into());}
    //            };


    //             for res in self.asm.map.values_mut() {
    //                 let machine_file = match res {
    //                     Ok(x) => x,
    //                     Err(e) => {
    //                         let error = StackedError::from_wraped(e.clone(),"while getting machine");
    //                         code_file.errors.push((error,None));
    //                         continue;
    //                     }
    //                 };
    //                 let ctx = match make_context(machine_file) {
    //                     Ok(x) =>x,
    //                     Err(e) => {
    //                         let error = StackedError::new(e,"while making context");
    //                         code_file.errors.push((error,None));
    //                         continue;
                            
    //                     }
    //                 };

    //                 for section in &machine_file.sections {
    //                     match section {
    //                         Section::Code(code_section) => {
    //                             for instruction in &code_section.instructions {

    //                                 match ctx.find_location(instruction.address) {
    //                                     Err(e) => {
    //                                         let error = StackedError::new(Box::new(e),"while reading instruction");
    //                                         code_file.errors.push((error,None));                                        
    //                                     }   
    //                                     Ok(None)=> {},
    //                                     Ok(Some(loc)) => 
                                    

    //                                     if let (Some(file_name),Some(line)) = (loc.file,loc.line){
    //                                         // let file = match fs::canonicalize(Path::new(file_name)) {
    //                                         //     Ok(file) =>file,
    //                                         //     Err(_) => {
    //                                         //         continue;
    //                                         //     },
    //                                         // };

    //                                         let file = Path::new(file_name);

    //                                         if *file!=*path {
    //                                             continue;
    //                                         }
    //                                         // let x = Instruction {
    //                                         //     detail: instruction.clone(),
    //                                         //     file: path.clone()
    //                                         // };

    //                                         code_file.asm.entry(line).or_default().push(instruction.clone());

    //                                     }

    //                                 }
    //                             }  
    //                         },
    //                         Section::Info(_) =>{}
    //                     }
    //                 }
                    
    //             }

                
    //             Ok(code_file)
    //         }
    //     }
    // }

    pub fn visit_machine_file(&mut self,path : Arc<Path>) -> Result<&mut MachineFile<'a>,Box<dyn Error>>{
        self.asm.get_machine(path)
    }
}
// impl CodeRegistry {
//     pub fn new() -> Self {
//         CodeRegistry::default()
//     }

//     pub fn get_source_file(&mut self,path : &Path) -> Result<&CodeFile,Box<dyn Error>>{
//         self.visit_source_file(path)?;
//         self.source_files.get(path).ok_or("failed to retrive source file".into())
//     }

//     pub fn visit_source_file(&mut self,path : &Path) -> Result<(),Box<dyn Error>>{
//         self._visit_source_file(path.into())
//     }

//     // pub fn _visit_source_file(&mut self,path : Arc<Path>) -> Result<(),Box<dyn Error>>{
//     //     if !self.visited.insert(path.clone()) {
//     //         return Ok(());
//     //     }

//     //     let f = CodeFile::read(&path)?;
//     //     self.source_files.insert(path,f);
//     //     Ok(())
//     // }
//     pub fn _visit_source_file(&mut self, path: Arc<Path>) -> Result<(), Box<dyn Error>> {
//         // Attempt to insert a new entry if the path is not already present
//         if let std::collections::hash_map::Entry::Vacant(e) = self.source_files.entry(path.clone()) {
//             let f = CodeFile::read(&path)?;
//             e.insert(f); // Insert the file into the map
//         }
//         Ok(())
//     }


//     pub fn visit_machine_file(&mut self,path : Arc<Path>
//         if !self.visited.insert(path.clone()) {
//             return Ok(());
//         }

//         //read and parse the file
//         let machine_file = asm.get_machine(path.clone()).map_err(|_e| "failed loading machine file")?;
//         let ctx = Context::from_dwarf(machine_file.load_dwarf().map_err(|_e| "failed load dwarf")?).map_err(|_e| "failed making ctx")?;


//         for section in &machine_file.sections {
//             match section {
//                 Section::Code(code_section) => {
//                     for instruction in &code_section.instructions {
                        
//                         //ignore missing but not invalid
//                         if let Some(loc) = ctx.find_location(instruction.address).map_err(|_e| "failed finding location")?{
//                             if let (Some(file_name),Some(line)) = (loc.file,loc.line){
                                
//                                 //TODO: this needs fixing!!!!
//                                 let file :Arc<Path>= match fs::canonicalize(Path::new(file_name)) {
//                                     Ok(file) =>file.into(),
//                                     Err(_) => {
//                                         println!("{}",format!("{:?} does not exist",file_name ).red());
//                                         continue;
//                                     },
//                                 };
//                                 if self._visit_source_file(file.clone()).is_err( ){
//                                     println!("{}", format!("failed finding related source file {}",file.to_string_lossy()).red());
//                                     continue;
//                                 } else {
//                                     println!("loaded {}",file.to_string_lossy());

//                                 }

//                                 // let file = match self.source_files.get_mut(&file) {
//                                 //     Some(file) => file,
//                                 //     None => {
//                                 //         //not sure how this can even triger
//                                 //         println!("error happened with {}",file.to_string_lossy());
//                                 //         continue;
//                                 //     }
//                                 // };

//                                 // self._visit_source_file(file.clone()).map_err(|_e| format!("failed finding related source files {}",file.to_string_lossy()))?;
//                                 let file = self.source_files.get_mut(&file).unwrap();

//                                 //insert
//                                 let x = Instruction {
//                                     detail: instruction.clone(),
//                                     file: path.clone()
//                                 };

//                                 file.asm.entry(line).or_default().push(x);

//                             }
//                         }
//                     }
//                 },
//                 _ => {},
//             }
//         } ;
//         // todo!()
//         Ok(())

//     }
// }



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