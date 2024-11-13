use gimli::RunTimeEndian;
use capstone::prelude::*;
use goblin::Object;
use gimli::{read::Dwarf, SectionId, EndianSlice};
use std::{fs, path::PathBuf, fmt};
use std::error::Error;

pub struct ParsedExecutable<'a> {
    pub sections: Vec<Section<'a>>,
    pub object: goblin::Object<'a>,
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

impl<'a> ParsedExecutable<'a> {
    pub fn parse(buffer: &'a[u8]) -> Result<ParsedExecutable, Box<dyn Error>> {
        // Parse goblin object
        let obj = Object::parse(buffer)?;

        match &obj {
            Object::Elf(elf) => {
                // Create Capstone instance dynamically
                let cs = create_capstone(&elf)?;
                let mut parsed_sections = Vec::new();

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
                        // Collect non-executable sections
                        parsed_sections.push(Section::NonExecutable(NonExecutableSection {
                            name: section_name,
                            data: section_data,
                        }));
                    }
                }

                // Determine the endianness dynamically
                let endian = if elf.little_endian { RunTimeEndian::Little } else { RunTimeEndian::Big };

                // Function to retrieve a section if present, or return an empty slice if absent.
                let get_section = |name: &str| {
                    elf.section_headers.iter().find_map(|s| {
                        if let Some(section_name) = elf.shdr_strtab.get_at(s.sh_name) {
                            if section_name == name {
                                return Some(&buffer[s.sh_offset as usize..(s.sh_offset + s.sh_size) as usize]);
                            }
                        }
                        None
                    }).unwrap_or(&[])
                };

                // Load DWARF sections
                let dwarf = Dwarf::load(|section| -> Result<EndianSlice<RunTimeEndian>, gimli::Error> {
                    let data = match section {
                        SectionId::DebugLine => get_section(".debug_line"),
                        SectionId::DebugInfo => get_section(".debug_info"),
                        SectionId::DebugAbbrev => get_section(".debug_abbrev"),
                        SectionId::DebugStr => get_section(".debug_str"),
                        SectionId::DebugRanges => get_section(".debug_ranges"),
                        SectionId::DebugRngLists => get_section(".debug_rnglists"),
                        SectionId::DebugAddr => get_section(".debug_addr"),
                        SectionId::DebugAranges => get_section(".debug_aranges"),
                        SectionId::DebugLoc => get_section(".debug_loc"),
                        SectionId::DebugLocLists => get_section(".debug_loclists"),
                        SectionId::DebugLineStr => get_section(".debug_line_str"),
                        SectionId::DebugStrOffsets => get_section(".debug_str_offsets"),
                        SectionId::DebugTypes => get_section(".debug_types"),
                        SectionId::DebugMacinfo => get_section(".debug_macinfo"),
                        SectionId::DebugMacro => get_section(".debug_macro"),
                        SectionId::DebugPubNames => get_section(".debug_pubnames"),
                        SectionId::DebugPubTypes => get_section(".debug_pubtypes"),
                        SectionId::DebugCuIndex => get_section(".debug_cu_index"),
                        SectionId::DebugTuIndex => get_section(".debug_tu_index"),
                        SectionId::DebugFrame => get_section(".debug_frame"),
                        SectionId::EhFrame => get_section(".eh_frame"),
                        SectionId::EhFrameHdr => get_section(".eh_frame_hdr"),
                    };

                    Ok(EndianSlice::new(data, endian))
                })?;

                Ok(ParsedExecutable {
                    sections: parsed_sections,
                    object: obj,
                    dwarf,
                })
            }
            _ => Err("Unsupported file format".into()),
        }
    }
}

fn create_capstone(elf: &goblin::elf::Elf) -> Result<Capstone, Box<dyn Error>> {
    let cs = match elf.header.e_machine {
        goblin::elf::header::EM_X86_64 => {
            Capstone::new().x86().mode(capstone::arch::x86::ArchMode::Mode64).build()?
        }
        goblin::elf::header::EM_386 => {
            Capstone::new().x86().mode(capstone::arch::x86::ArchMode::Mode32).build()?
        }
        goblin::elf::header::EM_ARM => {
            Capstone::new().arm().mode(capstone::arch::arm::ArchMode::Arm).build()?
        }
        // Add more architectures as needed
        _ => return Err("Unsupported architecture".into()),
    };
    Ok(cs)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <ELF FILE>", args[0]);
        std::process::exit(1);
    }
    
    let file_path = PathBuf::from(&args[1]);
    let buffer = fs::read(&file_path)?;

    let parsed_executable = ParsedExecutable::parse(&buffer)?;

    println!("Parsed ELF file: {}", file_path.display());
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

    Ok(())
}
