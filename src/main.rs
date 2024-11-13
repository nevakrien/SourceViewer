use gimli::RunTimeEndian;
use capstone::prelude::*;
use goblin::Object;
use gimli::{read::Dwarf,SectionId, EndianSlice};
use std::{env, fs, path::PathBuf};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <ELF FILE>", args[0]);
        std::process::exit(1);
    }
    
    let file_path = PathBuf::from(&args[1]);
    let buffer = fs::read(&file_path)?;

    match Object::parse(&buffer)? {
        Object::Elf(elf) => {
            println!("Parsing ELF file: {}", file_path.display());

            // Use Capstone to disassemble the sections containing executable code.
            let cs = Capstone::new().x86().mode(capstone::arch::x86::ArchMode::Mode64).build()?;
            for section in &elf.section_headers {
                if section.is_executable() {
                    let code = &buffer[section.sh_offset as usize..(section.sh_offset + section.sh_size) as usize];
                    let disasm = cs.disasm_all(code, section.sh_addr)?;
                    println!("Disassembly of section at offset 0x{:x}", section.sh_offset);
                    for insn in disasm.iter() {
                        println!("0x{:x}: {} {}", insn.address(), insn.mnemonic().unwrap_or(""), insn.op_str().unwrap_or(""));
                    }
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

            // Traverse and extract source lines from DWARF data here.
            println!("DWARF information parsed successfully.\n{:?}", dwarf);
            
            // Additional DWARF processing logic as needed for specific sections.
        }
        _ => {
            eprintln!("Unsupported file format");
            std::process::exit(1);
        }
    }

    Ok(())
}
