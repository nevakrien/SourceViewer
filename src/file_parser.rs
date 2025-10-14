use object::pe::IMAGE_SCN_MEM_EXECUTE;
use object::Architecture;
use object::{Object, ObjectSection, SectionFlags};
use std::cell::Cell;
use std::collections::btree_map;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use addr2line::Context;
use capstone::arch::{arm, arm64, x86};
use capstone::prelude::*;
use gimli::RunTimeEndian;
use gimli::{read::Dwarf, EndianSlice, SectionId};
use std::error::Error;
// pub type LineMap = BTreeMap<u32,Vec<InstructionDetail>>;
// pub type FileMap = HashMap<Arc<Path>,LineMap>;
#[derive(Debug, Default)]
pub struct LineMap {
    inner: BTreeMap<u32, Vec<InstructionDetail>>,
    extra: Vec<InstructionDetail>,
}

impl LineMap {
    #[inline(always)]
    pub fn get(&self, id: &u32) -> Option<&Vec<InstructionDetail>> {
        self.inner.get(id)
    }

    #[inline(always)]
    pub fn iter_maped(&'_ self) -> btree_map::Iter<'_, u32, Vec<InstructionDetail>> {
        self.inner.iter()
    }
}

#[derive(Debug, Default)]
pub struct FileMap {
    inner: HashMap<Arc<Path>, LineMap>,
    extra: Vec<InstructionDetail>,
}

impl FileMap {
    #[inline(always)]
    pub fn get(&self, id: &Arc<Path>) -> Option<&LineMap> {
        self.inner.get(id)
    }

    // fn entry(&mut self,id:Arc<Path>) -> hash_map::Entry<'_,Arc<Path>,LineMap> {
    //     self.inner.entry(id)
    // }
}

type Endian<'a> = EndianSlice<'a, RunTimeEndian>;

#[derive(Debug)]
pub struct MachineFileInner<'a> {
    pub obj: object::File<'a>,
    pub sections: Vec<Section<'a>>,
}

// #[derive(Debug)]
pub struct MachineFile<'a> {
    pub obj: object::File<'a>,
    pub sections: Vec<Section<'a>>,
    dwarf: Cell<Option<Arc<Dwarf<Endian<'a>>>>>,
    addr2line: Cell<Option<Arc<Context<Endian<'a>>>>>,
    file_lines: Cell<Option<Arc<FileMap>>>, //line -> instruction>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Section<'a> {
    Code(LazyCondeSection<'a>),
    Info(InfoSection<'a>),
}

impl Section<'_> {
    pub fn name(&self) -> &str {
        match self {
            Section::Code(x) => x.name(),
            Section::Info(x) => &x.name,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LazyCondeSection<'a> {
    Done(CodeSection),
    Pending(InfoSection<'a>),
}

impl LazyCondeSection<'_> {
    pub fn name(&self) -> &str {
        match self {
            LazyCondeSection::Done(x) => &x.name,
            LazyCondeSection::Pending(x) => &x.name,
        }
    }

    pub fn disasm(&mut self, arch: &Architecture) -> Result<(), Box<dyn Error>> {
        let LazyCondeSection::Pending(_code_section) = self else {
            return Ok(());
        };
        // Disassemble executable sections
        let mut cs = create_capstone(arch)?;
        self.disasm_capstone(&mut cs)
    }
    pub fn disasm_capstone(&mut self,cs:&mut Capstone) -> Result<(), Box<dyn Error>> {
        let LazyCondeSection::Pending(code_section) = self else {
            return Ok(());
        };
        let disasm = cs.disasm_all(code_section.data, code_section.address)?;
        let mut instructions = Vec::new();
        for (serial_number, insn) in disasm.iter().enumerate() {
            instructions.push(InstructionDetail {
                serial_number,
                address: insn.address(),
                mnemonic: insn
                    .mnemonic()
                    .unwrap_or("unknown")
                    .to_owned()
                    .into_boxed_str(),
                op_str: insn.op_str().unwrap_or("unknown").to_owned(),
                size: insn.len(),
            });
        }

        *self = LazyCondeSection::Done(CodeSection {
            name: code_section.name.clone(),
            instructions: instructions.into(),
        });

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CodeSection {
    pub name: Box<str>,
    pub instructions: Arc<[InstructionDetail]>,
}


pub struct NewCodeSection<'a>{
    pub name: Box<str>,
    pub data: &'a [u8],
    pub address: u64,
    cs:Capstone,
}

type Res =  Result<(),Box<dyn Error>>;

impl NewCodeSection<'_>{
    pub fn map_chunk(&self,start:u64,len:u64,f:&mut impl FnMut(InstructionDetail)->Res)->Res{
        let end = (start+len) as usize;
        let s = start as usize;
        self._map_chunk(&self.data[s..end],start,f)
    }
    fn _map_chunk(&self,data:&[u8],address:u64,f:&mut impl FnMut(InstructionDetail)->Res)-> Res{
        let disasm = self.cs.disasm_all(data, address)?;
        for (serial_number, insn) in disasm.iter().enumerate() {
            f(InstructionDetail {
                serial_number,
                address: insn.address(),
                mnemonic: insn
                    .mnemonic()
                    .unwrap_or("unknown")
                    .to_owned()
                    .into_boxed_str(),
                op_str: insn.op_str().unwrap_or("unknown").to_owned(),
                size: insn.len(),
            })?;
        }
        Ok(())
    }
    fn _disasm_chunk(&self,data:&[u8],address:u64) -> Result<Vec<InstructionDetail>, Box<dyn Error>>{
        let mut instructions = Vec::new();
        self._map_chunk(data, address,&mut |x|{Ok(instructions.push(x))})?;
        Ok(instructions)

    }
}
    

#[derive(Clone, Debug, PartialEq)]
pub struct InfoSection<'a> {
    pub name: Box<str>,
    pub data: &'a [u8],
    pub address: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InstructionDetail {
    pub serial_number: usize,

    pub address: u64,
    pub mnemonic: Box<str>,
    pub op_str: String,
    pub size: usize,
}

impl<'a> MachineFile<'a> {
    pub fn get_lines_map(&mut self) -> Result<Arc<FileMap>, Box<dyn Error>> {
        if let Some(ans) = self.file_lines.replace(None) {
            self.file_lines.set(Some(ans.clone()));
            return Ok(ans);
        }

        let context = self.get_addr2line()?;

        // let mut ans = Arc::new(HashMap::new());
        let mut ans = Arc::new(FileMap::default());
        let handle = Arc::get_mut(&mut ans).unwrap();

        for code_section in self.sections.iter_mut().filter_map(|item| {
            if let Section::Code(c) = item {
                Some(c)
            } else {
                None
            }
        }) {
            code_section.disasm(&self.obj.architecture())?;
            let LazyCondeSection::Done(code) = code_section else {
                unreachable!()
            };
            for instruction in code.instructions.iter() {
                if let Ok(Some(loc)) = context.find_location(instruction.address) {
                    match (loc.file, loc.line) {
                        (Some(file_name), Some(line)) => {
                            let file = Path::new(file_name).into();

                            handle
                                .inner
                                .entry(file)
                                .or_default()
                                .inner
                                .entry(line)
                                .or_default()
                                .push(instruction.clone());
                        }
                        (Some(file_name), None) => {
                            let file = Path::new(file_name).into();

                            handle
                                .inner
                                .entry(file)
                                .or_default()
                                .extra
                                .push(instruction.clone());
                        }
                        (None, _) => todo!(),
                    }
                } else {
                    handle.extra.push(instruction.clone())
                }
            }
        }

        self.file_lines.set(Some(ans.clone()));
        Ok(ans)
    }

    fn get_gimli_section(&self, section: SectionId) -> &'a [u8] {
        self.obj
            .section_by_name(section.name())
            .and_then(|x| x.data().ok())
            .unwrap_or(&[])
    }

    pub fn load_dwarf(&self) -> Result<Arc<Dwarf<Endian<'a>>>, gimli::Error> {
        if let Some(dwarf) = self.dwarf.replace(None) {
            self.dwarf.set(Some(dwarf.clone()));
            return Ok(dwarf);
        }

        let endian = if self.obj.is_little_endian() {
            RunTimeEndian::Little
        } else {
            RunTimeEndian::Big
        };
        let dwarf = Dwarf::load(
            |section| -> Result<EndianSlice<RunTimeEndian>, gimli::Error> {
                Ok(EndianSlice::new(self.get_gimli_section(section), endian))
            },
        );

        let dwarf = Arc::new(dwarf?);

        self.dwarf.set(Some(dwarf.clone()));
        Ok(dwarf)
    }

    pub fn get_addr2line(&self) -> Result<Arc<Context<Endian<'a>>>, Box<dyn Error>> {
        if let Some(addr) = self.addr2line.replace(None) {
            self.addr2line.set(Some(addr.clone()));
            return Ok(addr);
        }
        self.addr2line
            .set(Some(Context::from_arc_dwarf(self.load_dwarf()?)?.into()));
        self.get_addr2line()
    }

    pub fn parse(buffer: &'a [u8],parse_asm:bool) -> Result<MachineFile<'a>, Box<dyn Error>> {
        let obj = object::File::parse(buffer)?;
        let arch = obj.architecture();
        let mut parsed_sections = Vec::new();

        for section in obj.sections() {
            let section_name: Box<str> = section.name()?.into();
            let section_data = section.data()?;

            if should_disassemble(&section) {
               parsed_sections.push(Section::Code(LazyCondeSection::Pending(
                    InfoSection{
                        name: section_name,
                        data: section_data,
                        address: section.address(),

                    }
                )));
                
            } else {
                // Collect non-executable sections
                parsed_sections.push(Section::Info(InfoSection {
                    name: section_name,
                    data: section_data,
                    address: section.address(),
                }));
            }
        }

        let mut ans = MachineFile {
            obj,
            sections: parsed_sections,
            dwarf: None.into(),
            addr2line: None.into(),
            file_lines: None.into(),
        };

        if parse_asm {
            let need_ctx = false;

            match(ans.get_addr2line(),need_ctx){
                (Ok(_ctx),_)=>{
                    slow_compile(&mut ans,&arch)?;

                },
                (Err(e),true)=>return Err(e),
                (Err(_e),false)=>{
                    //slow fallback
                    // eprintln!("⚠️ failed to retrive dwarf info, runing slow single thread disassembler");
                    slow_compile(&mut ans,&arch)?;
                    
                }
            }
            
        }
        Ok(ans)
    }
}

#[inline(always)]
fn slow_compile(ans:&mut MachineFile,arch:&Architecture)->Result<(),Box<dyn Error>>{
    let mut cs = create_capstone(&arch)?;
    for s in ans.sections.iter_mut(){
        match s {
            Section::Code(c)=> c.disasm_capstone(&mut cs)?,
            _=>{},
        }
    }
    Ok(())
}

fn create_capstone(arch: &object::Architecture) -> Result<Capstone, Box<dyn Error>> {
    let mut cs = match arch {
        object::Architecture::X86_64 => Capstone::new()
            .x86()
            .mode(x86::ArchMode::Mode64)
            .detail(false)
            .build()?,
        object::Architecture::I386 => Capstone::new()
            .x86()
            .mode(x86::ArchMode::Mode32)
            .detail(false)
            .build()?,
        object::Architecture::Arm => Capstone::new()
            .arm()
            .mode(arm::ArchMode::Arm)
            .detail(false)
            .build()?,
        object::Architecture::Aarch64 => Capstone::new()
            .arm64()
            .mode(arm64::ArchMode::Arm)
            .detail(false)
            .build()?,
        object::Architecture::Riscv64 => Capstone::new()
            .riscv()
            .mode(capstone::arch::riscv::ArchMode::RiscV64)
            .detail(false)
            .build()?,

        object::Architecture::Riscv32 => Capstone::new()
            .riscv()
            .mode(capstone::arch::riscv::ArchMode::RiscV32)
            .detail(false)
            .build()?,

        object::Architecture::Mips64 => Capstone::new()
            .mips()
            .mode(capstone::arch::mips::ArchMode::Mips64)
            .detail(false)
            .build()?,
        object::Architecture::PowerPc => Capstone::new()
            .ppc()
            .mode(capstone::arch::ppc::ArchMode::Mode32)
            .detail(false)
            .build()?,
        object::Architecture::PowerPc64 => Capstone::new()
            .ppc()
            .mode(capstone::arch::ppc::ArchMode::Mode64)
            .detail(false)
            .build()?,
        object::Architecture::Sparc => Capstone::new()
            .sparc()
            .mode(capstone::arch::sparc::ArchMode::Default)
            .detail(false)
            .build()?,

        // Add more architectures as needed
        _ => return Err("Unsupported architecture".into()),
    };
    cs.set_skipdata(true)?;
    Ok(cs)
}

fn should_disassemble(sec: &object::Section) -> bool {
    match sec.flags() {
        // Check for ELF executable flag
        SectionFlags::Elf { sh_flags } => {
            // Executable sections in ELF usually have the `SHF_EXECINSTR` flag set (0x4).
            // `object::elf::SHF_EXECINSTR` is a constant representing this flag.
            sh_flags & object::elf::SHF_EXECINSTR as u64 != 0
        }
        // Check for Mach-O executable flag
        SectionFlags::MachO { flags } => {
            // Mach-O executables sections typically have the `S_ATTR_PURE_INSTRUCTIONS` attribute set.
            // `object::macho::S_ATTR_PURE_INSTRUCTIONS` is a constant representing this flag.
            flags & object::macho::S_ATTR_PURE_INSTRUCTIONS != 0
        }
        // Check for COFF executable flag
        SectionFlags::Coff { characteristics } => {
            // COFF executable sections have the `IMAGE_SCN_MEM_EXECUTE` characteristic set.
            // `object::coff::IMAGE_SCN_MEM_EXECUTE` is a constant representing this flag.
            characteristics & IMAGE_SCN_MEM_EXECUTE != 0
        }

        // Default case for any unsupported section flags
        SectionFlags::None => false,
        _ => todo!(),
    }
}

