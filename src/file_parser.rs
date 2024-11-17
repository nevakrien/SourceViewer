use object::pe::IMAGE_SCN_MEM_EXECUTE;
use object::ObjectSection;
use object::Object;
// use goblin::pe;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
// use goblin::mach;
// use goblin::mach::Mach;
// use goblin::elf;
use gimli::RunTimeEndian;
use capstone::arch::{arm, arm64, x86};
use capstone::prelude::*;
// use goblin::Object;
use gimli::{read::Dwarf, SectionId, EndianSlice};
use std::{fmt};
use std::error::Error;

#[derive(Clone,Debug,PartialEq)]
pub struct MachineFile<'a> {
    pub arch: object::Architecture,
    pub sections: Vec<Section<'a>>,
    // pub object: goblin::Object<'a>,
    // pub dwarf: gimli::read::Dwarf<EndianSlice<'a, RunTimeEndian>>,
    pub dwarf_loader: DwarfSectionLoader<'a>
}

#[derive(Clone,Debug,PartialEq)]
pub enum Section<'a> {
    Code(CodeSection),
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

#[derive(Clone,Debug,PartialEq)]
pub struct CodeSection{
    pub name: Box<str>,
    pub instructions: Vec<InstructionDetail>,
}

#[derive(Clone,Debug,PartialEq)]
pub struct InfoSection<'a> {
    pub name: Box<str>,
    pub data: &'a[u8],
}

#[derive(Clone,Debug,PartialEq)]
pub struct InstructionDetail {
    pub address: u64,
    pub mnemonic: Box<str>,
    pub op_str: Box<str>,
    pub size: usize,

    // pub read_regs : Box<[RegId]>,
    // pub write_regs : Box<[RegId]>,
    // pub groups: Box<[InsnGroupId]>,
}

// impl InstructionDetail {
//     /// Check if the instruction belongs to a specific group
//     pub fn has_group(&self, group: u32) -> bool {
//         self.groups.iter().any(|&g| g == InsnGroupId(group as u8))
//     }
// }


// Define a struct to hold DWARF section data
#[derive(Clone,Debug,PartialEq)]
pub struct DwarfSectionLoader<'a> {
    pub sections: HashMap<SectionId, &'a [u8]>,
    endian: RunTimeEndian,
}


#[derive(Debug,Clone)]
struct DuplicateEntry;

impl fmt::Display for DuplicateEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Duplicate entry detected")
    }
}

impl std::error::Error for DuplicateEntry {}

impl<'a> DwarfSectionLoader<'a> {
    fn new(endian: RunTimeEndian) -> Self {
        Self {
            sections: HashMap::new(),
            endian,
        }
    }

    

    // Method to add a section if it matches one of the DWARF sections
    fn maybe_add_section(&mut self, section_name: &str, data: &'a [u8]) -> Result<bool,DuplicateEntry>{
        let section_id = match section_name {
            ".debug_line" => Some(SectionId::DebugLine),
            ".debug_info" => Some(SectionId::DebugInfo),
            ".debug_abbrev" => Some(SectionId::DebugAbbrev),
            ".debug_str" => Some(SectionId::DebugStr),
            ".debug_ranges" => Some(SectionId::DebugRanges),
            ".debug_rnglists" => Some(SectionId::DebugRngLists),
            ".debug_addr" => Some(SectionId::DebugAddr),
            ".debug_aranges" => Some(SectionId::DebugAranges),
            ".debug_loc" => Some(SectionId::DebugLoc),
            ".debug_loclists" => Some(SectionId::DebugLocLists),
            ".debug_line_str" => Some(SectionId::DebugLineStr),
            ".debug_str_offsets" => Some(SectionId::DebugStrOffsets),
            ".debug_types" => Some(SectionId::DebugTypes),
            ".debug_macinfo" => Some(SectionId::DebugMacinfo),
            ".debug_macro" => Some(SectionId::DebugMacro),
            ".debug_pubnames" => Some(SectionId::DebugPubNames),
            ".debug_pubtypes" => Some(SectionId::DebugPubTypes),
            ".debug_cu_index" => Some(SectionId::DebugCuIndex),
            ".debug_tu_index" => Some(SectionId::DebugTuIndex),
            ".debug_frame" => Some(SectionId::DebugFrame),
            ".eh_frame" => Some(SectionId::EhFrame),
            ".eh_frame_hdr" => Some(SectionId::EhFrameHdr),

            //mising cases
            ".debug_names"| ".debug_sup" | ".debug_str_sup" => todo!(),
            ".gdb_index" | ".debug_gnu_pubnames" | ".debug_gnu_pubtypes" => todo!(),


            name => if name.starts_with(".zdebug_") {todo!()} else {None},
        };

        if let Some(id) = section_id {
            match self.sections.entry(id) {
                Entry::Vacant(entry) => {
                    entry.insert(data);
                    Ok(true)
                }
                Entry::Occupied(_) => Err(DuplicateEntry),
            }
        } else {
            Ok(false)
        }
    }

    // Method to retrieve a section's data by its DWARF SectionId
    pub fn get_section(&self, section: SectionId) -> &'a [u8] {
        self.sections.get(&section).copied().unwrap_or(&[])
    }

    // Method to load DWARF data using the stored sections
    pub fn load_dwarf(&self) -> Result<Dwarf<EndianSlice<'a,RunTimeEndian>>, gimli::Error> {
        Dwarf::load(|section| -> Result<EndianSlice<RunTimeEndian>, gimli::Error> {
            Ok(EndianSlice::new(self.get_section(section), self.endian))
        })
    }
}

impl<'a> MachineFile<'a> {
    pub fn load_dwarf(&self) -> Result<Dwarf<EndianSlice<'a,RunTimeEndian>>, gimli::Error>{
        self.dwarf_loader.load_dwarf()
    }

    pub fn parse(buffer: &'a[u8]) -> Result<MachineFile, Box<dyn Error>>{
        let obj = object::File::parse(buffer)?;
        let arch = obj.architecture();
        let endian = if obj.is_little_endian() { RunTimeEndian::Little } else { RunTimeEndian::Big };
        let mut cs = create_capstone(&arch)?;
        cs.set_skipdata(true)?;
        let mut parsed_sections = Vec::new();
        let mut dw = DwarfSectionLoader::new(endian);
        
        for section in obj.sections() {
            let section_name :Box<str>= section.name()?.into();
            let section_data = section.data()?;

            if should_disassemble(&section) {
                // Disassemble executable sections
                let disasm = cs.disasm_all(section_data, section.address())?;
                let mut instructions = Vec::new();
                for insn in disasm.iter() {
                    // let detail =  cs.insn_detail(insn)?;
                    instructions.push(InstructionDetail {
                        address: insn.address(),
                        mnemonic: insn.mnemonic().unwrap_or("unknown").to_owned().into_boxed_str(),
                        op_str: insn.op_str().unwrap_or("unknown").to_owned().into_boxed_str(),
                        size: insn.len(),

                        // groups: detail.groups().into(),
                        // write_regs: detail.regs_write().into(),
                        // read_regs: detail.regs_read().into(),
                    });
                }

                parsed_sections.push(Section::Code(CodeSection {
                    name: section_name,
                    instructions,
                }));
            } else {
                dw.maybe_add_section(&section_name,section_data)?;
                // Collect non-executable sections
                parsed_sections.push(Section::Info(InfoSection {
                    name: section_name,
                    data: section_data,
                }));
            }

        }
        Ok(MachineFile {
            arch,
            sections: parsed_sections,
            // object: obj,
            dwarf_loader:dw,
        })
    }

}

fn create_capstone(arch: &object::Architecture) -> Result<Capstone, Box<dyn Error>> {
    let cs = match arch {
        object::Architecture::X86_64 => {
            Capstone::new().x86().mode(x86::ArchMode::Mode64).detail(false).build()?
        }
        object::Architecture::I386 => {
            Capstone::new().x86().mode(x86::ArchMode::Mode32).detail(false).build()?
        }
        object::Architecture::Arm => {
            Capstone::new().arm().mode(arm::ArchMode::Arm).detail(false).build()?
        }
        object::Architecture::Aarch64 => {
            Capstone::new().arm64().mode(arm64::ArchMode::Arm).detail(false).build()?
        }
        object::Architecture::Riscv64 => {
            Capstone::new().riscv().mode(capstone::arch::riscv::ArchMode::RiscV64).detail(false).build()?
        }

        object::Architecture::Riscv32 => {
            Capstone::new().riscv().mode(capstone::arch::riscv::ArchMode::RiscV32).detail(false).build()?
        }

        object::Architecture::Mips64 => {
            Capstone::new().mips().mode(capstone::arch::mips::ArchMode::Mips64).detail(false).build()?
        }
        object::Architecture::PowerPc => {
            Capstone::new().ppc().mode(capstone::arch::ppc::ArchMode::Mode32).detail(false).build()?
        }
        object::Architecture::PowerPc64 => {
            Capstone::new().ppc().mode(capstone::arch::ppc::ArchMode::Mode64).detail(false).build()?
        }
        object::Architecture::Sparc => {
            Capstone::new().sparc().mode(capstone::arch::sparc::ArchMode::Default).detail(false).build()?
        }

        // Add more architectures as needed
        _ => return Err("Unsupported architecture".into()),
    };
    Ok(cs)
}



use object::{SectionFlags};

fn should_disassemble(sec: &object::Section) -> bool {
    match sec.flags() {
        // Check for ELF executable flag
        SectionFlags::Elf { sh_flags } => {
            // Executable sections in ELF usually have the `SHF_EXECINSTR` flag set (0x4).
            // `object::elf::SHF_EXECINSTR` is a constant representing this flag.
            sh_flags & object::elf::SHF_EXECINSTR as u64 != 0
        },
        // Check for Mach-O executable flag
        SectionFlags::MachO { flags } => {
            // Mach-O executables sections typically have the `S_ATTR_PURE_INSTRUCTIONS` attribute set.
            // `object::macho::S_ATTR_PURE_INSTRUCTIONS` is a constant representing this flag.
            flags & object::macho::S_ATTR_PURE_INSTRUCTIONS != 0
        },
        // Check for COFF executable flag
        SectionFlags::Coff { characteristics } => {
            // COFF executable sections have the `IMAGE_SCN_MEM_EXECUTE` characteristic set.
            // `object::coff::IMAGE_SCN_MEM_EXECUTE` is a constant representing this flag.
            characteristics & IMAGE_SCN_MEM_EXECUTE != 0
        },

        // Default case for any unsupported section flags
        SectionFlags::None => false,
        _ => todo!(),
    }
}
