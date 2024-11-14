// use goblin::elf::header;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use goblin::mach;
use goblin::mach::Mach;
use goblin::elf;
use gimli::RunTimeEndian;
use capstone::arch::{arm, arm64, x86};
use capstone::prelude::*;
use goblin::Object;
use gimli::{read::Dwarf, SectionId, EndianSlice};
use std::{fs, path::PathBuf, fmt};
use std::error::Error;

pub struct ParsedExecutable<'a> {
    pub sections: Vec<Section<'a>>,
    // pub object: goblin::Object<'a>,
    pub dwarf: gimli::read::Dwarf<EndianSlice<'a, RunTimeEndian>>,
}

pub enum Section<'a> {
    Code(CodeSection),
    NonExecutable(NonExecutableSection<'a>),
}

pub struct CodeSection{
    pub name: Box<str>,
    pub instructions: Vec<InstructionDetail>,
}

pub struct NonExecutableSection<'a> {
    pub name: Box<str>,
    pub data: &'a[u8],
}

pub struct InstructionDetail {
    pub address: u64,
    pub mnemonic: Box<str>,
    pub op_str: Box<str>,
}

impl fmt::Display for InstructionDetail {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#010x}: {} {}", self.address, self.mnemonic, self.op_str)
    }
}

// Define a struct to hold DWARF section data
struct DwarfSectionLoader<'a> {
    sections: HashMap<SectionId, &'a [u8]>,
    endian: RunTimeEndian,
}

#[derive(Debug)]
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
    fn maybe_add_section(&mut self, section_name: &str, data: &'a [u8]) -> Result<(),DuplicateEntry>{
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
            _ => None,
        };

        if let Some(id) = section_id {
            match self.sections.entry(id) {
                Entry::Vacant(entry) => {
                    entry.insert(data);
                    Ok(())
                }
                Entry::Occupied(_) => Err(DuplicateEntry),
            }
        } else {
            Ok(())
        }
    }

    // Method to retrieve a section's data by its DWARF SectionId
    fn get_section(&self, section: SectionId) -> &'a [u8] {
        self.sections.get(&section).map(|x| *x).unwrap_or(&[])
    }

    // Method to load DWARF data using the stored sections
    fn load_dwarf(&self) -> Result<Dwarf<EndianSlice<'a,RunTimeEndian>>, gimli::Error> {
        Dwarf::load(|section| -> Result<EndianSlice<RunTimeEndian>, gimli::Error> {
            Ok(EndianSlice::new(self.get_section(section), self.endian))
        })
    }
}

impl<'a> ParsedExecutable<'a> {
    pub fn parse(buffer: &'a[u8]) -> Result<ParsedExecutable, Box<dyn Error>> {
        // Parse goblin object
        let obj = Object::parse(buffer)?;

        match &obj {
            Object::Elf(elf) => {Self::parse_elf(elf,buffer)},
            Object::Mach(mach) => {Self::parse_mach(mach,buffer)},
            _ => Err("Unsupported file format".into()),
        }
    }

    pub fn parse_elf(elf:&elf::Elf,buffer: &'a[u8]) -> Result<ParsedExecutable<'a>, Box<dyn Error>> {
        // Determine the endianness dynamically
        let endian = if elf.little_endian { RunTimeEndian::Little } else { RunTimeEndian::Big };

        // Create Capstone instance dynamically
        let cs = create_capstone_elf(&elf)?;
        let mut parsed_sections = Vec::new();
        let mut dw = DwarfSectionLoader::new(endian);

        // Process sections in the order they come in the ELF file
        for section in &elf.section_headers {
            let section_name = elf.shdr_strtab.get_at(section.sh_name).unwrap_or("unknown").to_string().into_boxed_str();
            let section_data = &buffer[section.sh_offset as usize..(section.sh_offset + section.sh_size) as usize];

            if section.is_executable() {
                // Disassemble executable sections
                let disasm = cs.disasm_all(section_data, section.sh_addr)?;
                let mut instructions = Vec::new();
                for insn in disasm.iter() {
                    instructions.push(InstructionDetail {
                        address: insn.address(),
                        mnemonic: insn.mnemonic().unwrap_or("unknown").to_owned().into_boxed_str(),
                        op_str: insn.op_str().unwrap_or("unknown").to_owned().into_boxed_str(),
                    });
                }

                parsed_sections.push(Section::Code(CodeSection {
                    name: section_name,
                    instructions,
                }));
            } else {
                dw.maybe_add_section(&section_name,section_data)?;
                // Collect non-executable sections
                parsed_sections.push(Section::NonExecutable(NonExecutableSection {
                    name: section_name,
                    data: section_data,
                }));
            }
        }

        let dwarf = dw.load_dwarf()?;

        Ok(ParsedExecutable {
            sections: parsed_sections,
            // object: obj,
            dwarf,
        })
    }

    pub fn parse_mach(mach: &Mach<'_>, buffer: &'a [u8]) -> Result<ParsedExecutable<'a>, Box<dyn Error>> {
        // Determine endianness (Mach-O defaults to little-endian on macOS)
        let endian = RunTimeEndian::Little;

        let cs = create_capstone_mach(&mach)?;
        let mut parsed_sections = Vec::new();
        let mut dw = DwarfSectionLoader::new(endian);

        // Iterate over Mach-O segments and sections
        if let Mach::Binary(mach_bin) = mach {
            for segment in &mach_bin.segments {
                for (section, _section_data) in segment.sections()? {
                    let section_name = section.name().unwrap_or("unknown").to_string().into_boxed_str();
                    let section_data = &buffer[section.offset as usize..(section.offset as u64 + section.size) as usize];

                    // Check if the section is executable based on Mach-O flags
                    if section.flags & mach::constants::SECTION_TYPE == mach::constants::S_REGULAR &&
                       section.flags & mach::constants::S_ATTR_PURE_INSTRUCTIONS != 0 {
                        // Disassemble executable sections
                        let disasm = cs.disasm_all(section_data, section.addr)?;
                        let mut instructions = Vec::new();
                        for insn in disasm.iter() {
                            instructions.push(InstructionDetail {
                                address: insn.address(),
                                mnemonic: insn.mnemonic().unwrap_or("unknown").to_owned().into_boxed_str(),
                                op_str: insn.op_str().unwrap_or("unknown").to_owned().into_boxed_str(),
                            });
                        }

                        parsed_sections.push(Section::Code(CodeSection {
                            name: section_name,
                            instructions,
                        }));
                    } else {
                        dw.maybe_add_section(&section_name,section_data)?;

                        // Collect non-executable sections
                        parsed_sections.push(Section::NonExecutable(NonExecutableSection {
                            name: section_name,
                            data: section_data,
                        }));
                    }
                }
            }

            let dwarf = dw.load_dwarf()?;

            Ok(ParsedExecutable {
                sections: parsed_sections,
                dwarf,
            })
        } else {
            Err("Unsupported Mach-O format".into())
        }
    }

}


fn create_capstone_elf(elf: &elf::Elf) -> Result<Capstone, Box<dyn Error>> {
    let cs = match elf.header.e_machine {
        elf::header::EM_X86_64 => {
            Capstone::new().x86().mode(x86::ArchMode::Mode64).build()?
        }
        elf::header::EM_386 => {
            Capstone::new().x86().mode(x86::ArchMode::Mode32).build()?
        }
        elf::header::EM_ARM => {
            Capstone::new().arm().mode(arm::ArchMode::Arm).build()?
        }
        elf::header::EM_AARCH64 => {
            Capstone::new().arm64().mode(arm64::ArchMode::Arm).build()?
        }
        elf::header::EM_RISCV => {
            Capstone::new().riscv().mode(capstone::arch::riscv::ArchMode::RiscV64).build()?
        }
        elf::header::EM_MIPS => {
            Capstone::new().mips().mode(capstone::arch::mips::ArchMode::Mips64).build()?
        }
        // Add more architectures as needed
        _ => return Err("Unsupported architecture".into()),
    };
    Ok(cs)
}

// Update create_capstone_mach function to match Mach-O CPU types
fn create_capstone_mach(mach: &Mach<'_>) -> Result<Capstone, Box<dyn Error>> {
    let cs = match mach {
        Mach::Binary(mach_bin) => {
            match mach_bin.header.cputype {
                goblin::mach::constants::cputype::CPU_TYPE_X86_64 => {
                    Capstone::new().x86().mode(x86::ArchMode::Mode64).build()?
                }
                goblin::mach::constants::cputype::CPU_TYPE_X86 => {
                    Capstone::new().x86().mode(x86::ArchMode::Mode32).build()?
                }
                goblin::mach::constants::cputype::CPU_TYPE_ARM => {
                    Capstone::new().arm().mode(arm::ArchMode::Arm).build()?
                }
                goblin::mach::constants::cputype::CPU_TYPE_ARM64 => {
                    Capstone::new().arm64().mode(arm64::ArchMode::Arm).build()?
                }
                _ => return Err("Unsupported architecture".into()),
            }
        }
        _ => return Err("Unsupported Mach-O format".into()),
    };
    Ok(cs)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <ASM FILE>", args[0]);
        std::process::exit(1);
    }
    
    let file_path = PathBuf::from(&args[1]);
    let buffer = fs::read(&file_path)?;

    let parsed_executable = ParsedExecutable::parse(&buffer)?;

    println!("Parsed file: {}", file_path.display());
    for section in &parsed_executable.sections {
        match section {
            Section::Code(code_section) => {
                println!("Code Section: {} ({} instructions)", code_section.name, code_section.instructions.len());
                for instruction in &code_section.instructions {
                    println!("  {}", instruction);
                }
            }
            Section::NonExecutable(non_exec_section) => {
                println!("Non-Executable Section: {} ({} bytes)", non_exec_section.name, non_exec_section.data.len());
            }
        }
    }
    println!("DWARF info loaded successfully");
    println!("{:?}", parsed_executable.dwarf);

    Ok(())
}
