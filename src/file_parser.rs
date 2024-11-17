use object::pe::IMAGE_SCN_MEM_EXECUTE;
use object::{Object,SectionFlags,ObjectSection};

use gimli::RunTimeEndian;
use capstone::arch::{arm, arm64, x86};
use capstone::prelude::*;
use gimli::{read::Dwarf, SectionId, EndianSlice};
use std::error::Error;

#[derive(Debug)]
pub struct MachineFile<'a> {
    pub obj: object::File<'a>,
    pub sections: Vec<Section<'a>>,
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
    pub op_str: String,
    pub size: usize,
}




impl<'a> MachineFile<'a> {
    pub fn get_gimli_section(&self, section: SectionId) -> &'a [u8] {
        self.obj.section_by_name(section.name()).map(|x| x.data().ok()).flatten().unwrap_or(&[])
    }

    pub fn load_dwarf(&self) -> Result<Dwarf<EndianSlice<'a,RunTimeEndian>>, gimli::Error>{
       let endian = if self.obj.is_little_endian() { RunTimeEndian::Little } else { RunTimeEndian::Big };
       Dwarf::load(|section| -> Result<EndianSlice<RunTimeEndian>, gimli::Error> {
            Ok(EndianSlice::new(self.get_gimli_section(section), endian))
        })
    }

    pub fn parse(buffer: &'a[u8]) -> Result<MachineFile, Box<dyn Error>>{
        let obj = object::File::parse(buffer)?;
        let arch = obj.architecture();
        let mut cs = create_capstone(&arch)?;
        cs.set_skipdata(true)?;
        let mut parsed_sections = Vec::new();
        
        for section in obj.sections() {
            let section_name :Box<str>= section.name()?.into();
            let section_data = section.data()?;

            if should_disassemble(&section) {
                // Disassemble executable sections
                let disasm = cs.disasm_all(section_data, section.address())?;
                let mut instructions = Vec::new();
                for insn in disasm.iter() {
                    instructions.push(InstructionDetail {
                        address: insn.address(),
                        mnemonic: insn.mnemonic().unwrap_or("unknown").to_owned().into_boxed_str(),
                        op_str: insn.op_str().unwrap_or("unknown").to_owned(),
                        size: insn.len(),


                    });
                }

                parsed_sections.push(Section::Code(CodeSection {
                    name: section_name,
                    instructions,
                }));
            } else {
                // Collect non-executable sections
                parsed_sections.push(Section::Info(InfoSection {
                    name: section_name,
                    data: section_data,
                }));
            }

        }
        Ok(MachineFile {
            obj,
            sections: parsed_sections,
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
