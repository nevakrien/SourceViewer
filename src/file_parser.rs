use crate::errors::StringError;
use object::pe::IMAGE_SCN_MEM_EXECUTE;
use object::Architecture;
use object::{Object, ObjectSection, SectionFlags};
use once_cell::unsync::OnceCell;
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
}

type Endian<'a> = EndianSlice<'a, RunTimeEndian>;

// #[derive(Debug)]
pub struct MachineFile<'a> {
    pub obj: object::File<'a>,
    pub sections: Box<[Section<'a>]>,
    dwarf: OnceCell<Arc<Dwarf<Endian<'a>>>>,
    addr2line: OnceCell<Arc<Context<Endian<'a>>>>,
    file_lines: OnceCell<Arc<FileMap>>, //line -> instruction>
}

#[derive(Clone, Debug, PartialEq)]
pub enum Section<'a> {
    Code(CodeSection<'a>),
    Info(InfoSection<'a>),
}

impl Section<'_> {
    pub fn name(&self) -> &str {
        match self {
            Section::Code(x) => &x.name,
            Section::Info(x) => &x.name,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CodeSection<'a> {
    pub name: Box<str>,
    pub data: &'a [u8],
    pub address: u64,
    asm: OnceCell<Arc<[InstructionDetail]>>,
}

// type Res =  Result<(),Box<dyn Error>>;

fn dissasm(
    cs: &Capstone,
    data: &[u8],
    address: u64,
) -> Result<Arc<[InstructionDetail]>, Box<dyn Error>> {
    let disasm = cs.disasm_all(data, address)?;
    let mut instructions = Vec::new();
    for (serial_number, insn) in disasm.iter().enumerate() {
        instructions.push(InstructionDetail {
            serial_number,
            address: insn.address(),
            mnemonic: insn.mnemonic().unwrap_or("unknown").into(),
            op_str: insn.op_str().unwrap_or("unknown").into(),
            size: insn.len(),
        });
    }
    Ok(instructions.into())
}

use std::collections::LinkedList;

fn merge_linked_lists(
    lists: Vec<LinkedList<Vec<InstructionDetail>>>,
) -> Vec<InstructionDetail> {

    struct Stream {
        next_block: Vec<InstructionDetail>,
        remaining_blocks: std::collections::linked_list::IntoIter<Vec<InstructionDetail>>,
    }

    // Build initial streams
    let mut streams: Vec<Stream> = lists
        .into_iter()
        .filter_map(|list| {
            let mut iterator = list.into_iter();
            iterator.next().map(|first_block| Stream {
                next_block: first_block,
                remaining_blocks: iterator,
            })
        })
        .collect();

    // Sort ascending by first address of the next_block
    streams.sort_by_key(|stream| stream.next_block.first().unwrap().address);

    let mut merged: Vec<InstructionDetail> = Vec::new();

    while let Some(mut stream) = streams.pop() {
        // Pop the smallest (streams is sorted ascending, so back is smallest)
        merged.extend(stream.next_block);

        // Advance this stream
        if let Some(next_block) = stream.remaining_blocks.next() {
            let first_address = next_block.first().unwrap().address;
            stream.next_block = next_block;

            // Manual insertion (bubble-up) to keep streams sorted ascending by address
            let mut insert_index = streams.len();
            while insert_index > 0
                && first_address < streams[insert_index - 1].next_block.first().unwrap().address
            {
                insert_index -= 1;
            }
            streams.insert(insert_index, stream);
        }
    }

    for (k,ins) in merged.iter_mut().enumerate(){
        ins.serial_number=k;
    }

    merged
}

fn dissasm_fast(
    arch: &Architecture,
    ctx:&Context<Endian<'_>>,
    data: &[u8],
    base_address: u64,
) -> Result<Arc<[InstructionDetail]>, Box<dyn Error>> {
use crossbeam_channel::TryRecvError;
    
    const STEP:u64 = 1024 * 1024;

//1. find reasonble start points
    let mut diffs = Vec::<(u64,u64)>::new();
    let mut cur_addr = base_address;
    let end_addr = base_address+data.len() as u64;
    
    // Always start the first region at the base
    diffs.push((base_address, 0));

    while cur_addr < end_addr {
        let next_probe = cur_addr + STEP;
        let Some(next_start) = get_past_valid(ctx, next_probe, end_addr)? else {
            break;
        };

        diffs.last_mut().unwrap().1 = next_start;
        diffs.push((next_start, 0));
        cur_addr = next_start;
    }

    //we allways end on last adress
    diffs.last_mut().unwrap().1=end_addr;

    //bail if its too small
    if diffs.len()<=1 {
        let cs = create_capstone(&arch)?;
        return dissasm(&cs,data,base_address);
    }

//2. run super fast on all
    use crossbeam_channel::unbounded;
    use std::thread;

    let num_threads = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);


    type Res = Result<LinkedList<Vec<InstructionDetail>>, Box<str>>;
    let (send_task,recive_task) = unbounded::<(u64,u64)>();//(num_threads*10);
    let (send_result,recive_result) = unbounded::<Res>();//(num_threads*10);

    
let all_results=thread::scope(|scope| -> Result<Vec<LinkedList<Vec<InstructionDetail>>>,Box<dyn Error>>{
    for _i in 0..num_threads {
        let recive = recive_task.clone();
        let send = send_result.clone();

        macro_rules! try_in_thread {
            ($expr:expr) => {
                match $expr {
                    Ok(x) => x,
                    Err(e) => {
                        // We can just send a String (or Box<str>) error message
                        let _ = send.send(Err(Box::<str>::from(format!("{e}"))));
                        return;
                    }
                }
            };
        }

        scope.spawn(move || {
            let cs = try_in_thread!(create_capstone(&arch).map_err(StringError::new));
            let mut ans = LinkedList::new();

            for (start, end) in recive {
                let this_data = &data[(start-base_address) as usize..][..(end - start) as usize];
                let disasm = try_in_thread!(cs.disasm_all(this_data, start));
                let mut instructions = Vec::new();

                for insn in disasm.iter() {
                    instructions.push(InstructionDetail {
                        serial_number: usize::MAX,
                        address: insn.address(),
                        mnemonic: insn.mnemonic().unwrap_or("unknown").into(),
                        op_str: insn.op_str().unwrap_or("unknown").into(),
                        size: insn.len(),
                    });
                }

                ans.push_back(instructions);
            }

            send.send(Ok(ans)).unwrap();
        });
    }

    drop(send_result);
    drop(recive_task);

    //2.5 now send the thing
    let mut ans = Vec::new();

    for task in diffs.into_iter(){
        send_task.send(task)?;
        //recive as we go so we dont deadlock on subtask panic
        match recive_result.try_recv() {
            Ok(Err(e)) => {
                return Err(Box::new(StringError::new(format!("{e}"))));
            }
            Ok(Ok(x)) => {
                ans.push(x);
            }
            Err(TryRecvError::Empty) => {
                // fine, just continue sending
            }
            Err(TryRecvError::Disconnected) => {
                return Err(Box::new(StringError::new("all worker threads disconnected early")));
            }
        }
    }
    drop(send_task);

    for res in recive_result {
        ans.push(res.map_err(|e|Box::new(StringError::new(format!("{e}"))))?);
    }


    Ok(ans)
})?;

if all_results.len()<num_threads{
    return Err(Box::new(StringError::new("worker thread died unexpectadly")));
}
   
Ok(merge_linked_lists(all_results).into())

}

impl CodeSection<'_> {
    pub fn get_existing_asm(&self) -> Arc<[InstructionDetail]> {
        self.asm.get().unwrap().clone()
    }
    pub fn get_asm(&self, arch: Architecture) -> Result<Arc<[InstructionDetail]>, Box<dyn Error>> {
        self.asm
            .get_or_try_init(|| {
                let cs = create_capstone(&arch)?;
                dissasm(&cs, self.data, self.address)
            })
            .cloned()
    }

    fn get_asm_capstone(
        &self,
        cs: &Capstone,
    ) -> Result<Arc<[InstructionDetail]>, Box<dyn Error>> {
        self.asm
            .get_or_try_init(|| dissasm(cs, self.data, self.address))
            .cloned()
    }

    pub fn get_asm_fast(
        &self,
        arch: &Architecture,
        ctx:&Context<Endian<'_>>,
    ) -> Result<Arc<[InstructionDetail]>, Box<dyn Error>> {
        self.asm
            .get_or_try_init(|| dissasm_fast(arch,ctx, self.data, self.address))
            .cloned()
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
    pub op_str: Box<str>,
    pub size: usize,
}

impl<'a> MachineFile<'a> {
    pub fn get_lines_map(&self) -> Result<Arc<FileMap>, Box<dyn Error>> {
        self.file_lines
            .get_or_try_init(|| {
                let context = self.get_addr2line()?;

                // let mut ans = Arc::new(HashMap::new());
                let mut ans = Arc::new(FileMap::default());
                let handle = Arc::get_mut(&mut ans).unwrap();

                for code_section in self.sections.iter().filter_map(|item| {
                    if let Section::Code(c) = item {
                        Some(c)
                    } else {
                        None
                    }
                }) {
                    for instruction in code_section.get_asm(self.obj.architecture())?.iter() {
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
                Ok(ans)
            })
            .cloned()
    }

    fn get_gimli_section(&self, section: SectionId) -> &'a [u8] {
        self.obj
            .section_by_name(section.name())
            .and_then(|x| x.data().ok())
            .unwrap_or(&[])
    }

    pub fn load_dwarf(&self) -> Result<Arc<Dwarf<Endian<'a>>>, gimli::Error> {
        self.dwarf
            .get_or_try_init(|| {
                let endian = if self.obj.is_little_endian() {
                    RunTimeEndian::Little
                } else {
                    RunTimeEndian::Big
                };
                Dwarf::load(
                    |section| -> Result<EndianSlice<RunTimeEndian>, gimli::Error> {
                        Ok(EndianSlice::new(self.get_gimli_section(section), endian))
                    },
                )
                .map(Arc::new)
            })
            .cloned()
    }

    pub fn get_addr2line(&self) -> Result<Arc<Context<Endian<'a>>>, Box<dyn Error>> {
        self.addr2line
            .get_or_try_init(|| Ok(Context::from_arc_dwarf(self.load_dwarf()?)?.into()))
            .cloned()
    }

    pub fn parse(buffer: &'a [u8], parse_asm: bool) -> Result<MachineFile<'a>, Box<dyn Error>> {
        let obj = object::File::parse(buffer)?;
        let arch = obj.architecture();
        let mut parsed_sections = Vec::new();

        for section in obj.sections() {
            let section_name: Box<str> = section.name()?.into();
            let section_data = section.data()?;

            if should_disassemble(&section) {
                parsed_sections.push(Section::Code(CodeSection {
                    name: section_name,
                    data: section_data,
                    address: section.address(),
                    asm: OnceCell::new(),
                }));
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
            sections: parsed_sections.into(),
            dwarf: OnceCell::new(),
            addr2line: OnceCell::new(),
            file_lines:OnceCell::new(),
        };

        if parse_asm {
            let need_ctx = false;

            match (ans.get_addr2line(), need_ctx) {
                (Ok(ctx), _) => {
                    fast_compile(&mut ans,&ctx, &arch)?;
                }
                (Err(e), true) => return Err(e),
                (Err(_e), false) => {
                    //slow fallback
                    eprintln!("⚠️ failed to retrive dwarf info, runing slow single thread disassembler");
                    slow_compile(&mut ans, &arch)?;
                }
            }
        }
        Ok(ans)
    }
}

#[inline(always)]
fn slow_compile(ans: &mut MachineFile, arch: &Architecture) -> Result<(), Box<dyn Error>> {
    let cs = create_capstone(arch)?;
    for s in ans.sections.iter_mut() {
        if let Section::Code(c) = s {
            c.get_asm_capstone(&cs)?;
        }
    }
    Ok(())
}

#[inline(always)]
fn fast_compile(ans: &mut MachineFile,ctx:&Context<Endian<'_>>, arch: &Architecture) -> Result<(), Box<dyn Error>> {
    for s in ans.sections.iter_mut() {
        if let Section::Code(c) = s {
            c.get_asm_fast(arch,ctx)?;
        }
    }
    Ok(())
}


fn get_past_valid(ctx:&Context<Endian<'_>>,start:u64,end:u64)->Result<Option<u64>,Box<dyn Error>>{
    let mut iter = ctx.find_location_range(start,end)?;
    if let Some((start,size,_)) = iter.next(){
        Ok(Some(start+size))
    }else{
        Ok(None)
    }
}

fn create_capstone(arch: &object::Architecture) -> Result<Capstone, capstone::Error> {
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
        _ => return Err(capstone::Error::CustomError("Unsupported architecture")),
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
