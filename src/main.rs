use capstone::prelude::*;
use goblin::Object;
use gimli::{read::Dwarf, EndianSlice, LittleEndian};
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

            // Extract DWARF sections and assume they are all required.
            let debug_info = elf.section_headers.iter().find_map(|s| {
                match elf.shdr_strtab.get_at(s.sh_name).unwrap() {
                    name if name == ".debug_info" => Some(&buffer[s.sh_offset as usize..(s.sh_offset + s.sh_size) as usize]),
                    _ => None,
                }
            }).expect("Missing .debug_info section");

            let debug_abbrev = elf.section_headers.iter().find_map(|s| {
                match elf.shdr_strtab.get_at(s.sh_name).unwrap() {
                    name if name == ".debug_abbrev" => Some(&buffer[s.sh_offset as usize..(s.sh_offset + s.sh_size) as usize]),
                    _ => None,
                }
            }).expect("Missing .debug_abbrev section");

            let debug_str = elf.section_headers.iter().find_map(|s| {
                match elf.shdr_strtab.get_at(s.sh_name).unwrap() {
                    name if name == ".debug_str" => Some(&buffer[s.sh_offset as usize..(s.sh_offset + s.sh_size) as usize]),
                    _ => None,
                }
            }).expect("Missing .debug_str section");

            // Create the Dwarf structure, explicitly providing the expected types.
            let dwarf: Dwarf<EndianSlice<LittleEndian>> = Dwarf::load(|section| -> Result<EndianSlice<LittleEndian>, gimli::Error> {
                match section {
                    gimli::SectionId::DebugInfo => Ok(EndianSlice::new(debug_info, LittleEndian)),
                    gimli::SectionId::DebugAbbrev => Ok(EndianSlice::new(debug_abbrev, LittleEndian)),
                    gimli::SectionId::DebugStr => Ok(EndianSlice::new(debug_str, LittleEndian)),
                    _ => Ok(EndianSlice::new(&[], LittleEndian)),
                }
            })?;

            // Traverse and extract source lines from DWARF data here.
            println!("DWARF information parsed successfully.\n{:?}",dwarf);
        }
        _ => {
            eprintln!("Unsupported file format");
            std::process::exit(1);
        }
    }

    Ok(())
}
