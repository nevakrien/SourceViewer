#![allow(non_upper_case_globals)]

use std::path::Path;
use typed_arena::Arena;
use crate::program_context::FileRegistry;
use std::error::Error;
use std::fs;
use crate::file_parser::MachineFile;
// use gimli::{DW_AT_call_origin,DW_AT_ranges,DW_AT_entry_pc,DW_AT_declaration,AttributeValue, Dwarf, Reader, DW_AT_high_pc, DW_AT_low_pc, DW_AT_name, DW_TAG_subprogram};

// /// Return (name, low_pc, high_pc)
// pub fn iter_function_ranges<R: Reader>(
//     dwarf: &Dwarf<R>
// ) -> gimli::Result<Vec<(Option<String>, u64, Option<u64>)>> {
//     let mut results = Vec::new();

//     let mut units = dwarf.units();
//     // println!("units {units:?}");


//     while let Some(header) = units.next()? {
//         // println!("header {header:?}");
//         let unit = dwarf.unit(header)?;

//         let mut entries = unit.entries();

//         while let Some((_, entry)) = entries.next_dfs()? {
//             // println!("entry {entry:?}");
//             if true {

//                 let mut low_pc: Option<u64> = None;
//                 let mut high_pc: Option<u64> = None;
//                 let mut name: Option<String> = None;

//                 let mut attrs = entry.attrs();
//                 while let Some(attr) = attrs.next()? {
//                     match attr.name() {
//                         DW_AT_entry_pc=>{
//                              println!("yay {:?}",attr.value());
//                         },
//                         DW_AT_declaration=>{
//                             // println!("yay {:?}",attr.value());
//                             break;
//                         },
//                         DW_AT_ranges=>{
//                             println!("yay {:?}",attr.value());
//                         },
//                         DW_AT_low_pc => {
//                             if let AttributeValue::Addr(addr) = attr.value() {
//                                 low_pc = Some(addr);
//                             }
//                         }
//                         DW_AT_high_pc => {
//                             match attr.value() {
//                                 // Absolute address form
//                                 AttributeValue::Addr(addr) => high_pc = Some(addr),
//                                 // Offset from low_pc
//                                 AttributeValue::Udata(offset) => {
//                                     if let Some(lp) = low_pc {
//                                         high_pc = Some(lp + offset);
//                                     }
//                                 }
//                                 _ => {}
//                             }
//                         }
//                         DW_AT_name => {
//                             if let Ok(s) = dwarf.attr_string(&unit, attr.value()) {
//                                 name = Some(s.to_string_lossy()?.into_owned());
//                             }
//                         },

//                         DW_AT_call_origin => {
//                             println!("yay");
//                             if let AttributeValue::UnitRef(off) = attr.value() {
//                                 if let Ok(origin_entry) = unit.entry(off) {
//                                     if let Some(name_attr_val) =
//                                         origin_entry.attr_value(DW_AT_name)?
//                                     {
//                                         if let Ok(s) = dwarf.attr_string(&unit, name_attr_val) {
//                                             name =
//                                                 Some(s.to_string_lossy()?.into_owned());
//                                         }
//                                     }
//                                 }
//                             }
//                         },
//                         _ => {}//{println!("whats this? {} {:?}",attr.name(),attr.value());}
//                     }
//                 }

//                 if let Some(lp) = low_pc {
//                     results.push((name, lp, high_pc));
//                 }
//             }else{
//                 // println!("whats this entry {}",entry.tag());
//             }   
//         }
//     }

//     Ok(results)
// }

use gimli::{
    AttributeValue, DebuggingInformationEntry, Dwarf, Reader, Unit, UnitOffset,
    DW_AT_call_origin, DW_AT_declaration, DW_AT_entry_pc, DW_AT_high_pc,
    DW_AT_low_pc, DW_AT_name, DW_AT_ranges,DW_AT_dwo_name,
};
use gimli::ReaderOffset;
use gimli::RangeListsOffset;


// pub fn iter_function_ranges<R: Reader<Offset = usize>>(
//     dwarf: &Dwarf<R>,
// ) -> gimli::Result<Vec<(Option<String>, u64, Option<u64>)>> {
//     let mut results = Vec::new();
//     let mut units = dwarf.units();

//     while let Some(header) = units.next()? {
//         let unit = dwarf.unit(header)?;
//         let mut entries = unit.entries();

//         while let Some((_, entry)) = entries.next_dfs()? {
//             let mut low_pc: Option<u64> = None;
//             let mut high_pc: Option<u64> = None;
//             let mut name: Option<String> = None;
//             let mut ranges_index: Option<u64> = None;

//             if let Some(attr) = entry.attr_value(gimli::DW_AT_dwo_name)? {
//                 if let gimli::AttributeValue::String(s) = attr {
//                     println!("Split DWARF detected: DW_AT_dwo_name = {}", s.to_string_lossy()?);
//                 } else {
//                     println!("Split DWARF detected: DW_AT_dwo_name (non-string form) = {:?}", attr);
//                 }
//             }

//             // Collect all attributes first
//             let mut attrs = entry.attrs();
//             while let Some(attr) = attrs.next()? {
//                 match attr.name() {
//                     DW_AT_low_pc => {
//                         if let AttributeValue::Addr(a) = attr.value() {
//                             low_pc = Some(a);
//                         }
//                     }
//                     DW_AT_high_pc => match attr.value() {
//                         AttributeValue::Addr(a) => high_pc = Some(a),
//                         AttributeValue::Udata(off) => {
//                             if let Some(lp) = low_pc {
//                                 high_pc = Some(lp + off);
//                             }
//                         }
//                         _ => {}
//                     },
//                     DW_AT_dwo_name=>{
//                         if let gimli::AttributeValue::String(s) = attr.value() {
//                             println!("Split DWARF detected: DW_AT_dwo_name = {}", s.to_string_lossy()?);
//                         } else {
//                             println!("Split DWARF detected: DW_AT_dwo_name (non-string form) = {:?}", attr);
//                         }
//                     }
//                     DW_AT_entry_pc => {
//                         if let AttributeValue::Addr(a) = attr.value() {
//                             low_pc.get_or_insert(a);
//                         }
//                     }
//                     DW_AT_ranges => {
//                         if let AttributeValue::RangeListsRef(r) = attr.value() {
//                             ranges_index = Some(r.0 as u64);
//                         }
//                     }
//                     DW_AT_name => {
//                         if let Ok(s) = dwarf.attr_string(&unit, attr.value()) {
//                             name = Some(s.to_string_lossy()?.into_owned());
//                         }
//                     }
//                     DW_AT_call_origin => {
//                         if let AttributeValue::UnitRef(off) = attr.value() {
//                             if let Ok(origin_entry) = unit.entry(off) {
//                                 if let Some(name_attr_val) =
//                                     origin_entry.attr_value(DW_AT_name)?
//                                 {
//                                     if let Ok(s) =
//                                         dwarf.attr_string(&unit, name_attr_val)
//                                     {
//                                         name = Some(s.to_string_lossy()?.into_owned());
//                                     }
//                                 }
//                             }
//                         }
//                     }
//                     DW_AT_declaration => {
//                         // Declaration-only DIE — no code
//                         break;
//                     }
//                     _ => {}
//                 }
//             }

//             // Print all address-bearing DIEs
//             if low_pc.is_some() || ranges_index.is_some() {
//                 println!("------------------------------------------------");
//                 println!(
//                     "Tag: {:?}, name: {:?}",
//                     entry.tag(),
//                     name.as_deref().unwrap_or("<anon>")
//                 );

//                 // print resolved addresses
//                 if let Some(lp) = low_pc {
//                     println!("  low_pc:  0x{lp:x}");
//                 }
//                 if let Some(hp) = high_pc {
//                     println!("  high_pc: 0x{hp:x}");
//                 }
//                 if let Some(idx) = ranges_index {
//                     println!("  ranges:  DebugRngListsIndex({idx})");

//                     // try to resolve and print actual ranges
//                     if let Some(offset) = ranges_index.map(|i| gimli::RangeListsOffset(i as usize)) {
//                         if let Ok(mut iter) = dwarf.ranges(&unit, offset) {
//                             while let Ok(Some(r)) = iter.next() {
//                                 println!("    range: [0x{:x} - 0x{:x})", r.begin, r.end);
//                             }
//                         }
//                     }
//                 }

//                 // print all attrs in case something interesting is hiding
//                 let mut attrs = entry.attrs();
//                 while let Some(attr) = attrs.next()? {
//                     let val_str = match attr.value() {
//                         AttributeValue::Addr(a) => format!("Addr(0x{a:x})"),
//                         AttributeValue::Udata(u) => format!("Udata({u})"),
//                         AttributeValue::Data1(d) => format!("Data1({d})"),
//                         AttributeValue::Data2(d) => format!("Data2({d})"),
//                         AttributeValue::Data4(d) => format!("Data4({d})"),
//                         AttributeValue::Data8(d) => format!("Data8({d})"),
//                         AttributeValue::String(s) => {
//                             format!("String({})", s.to_string_lossy()?.into_owned())
//                         }
//                         other => format!("{other:?}"),
//                     };
//                     println!("    {:?} = {}", attr.name(), val_str);
//                 }

//                 // keep record for caller
//                 if let Some(lp) = low_pc {
//                     results.push((name.clone(), lp, high_pc));
//                 }
//             }
//         }
//     }

//     Ok(results)
// }


pub fn iter_function_ranges<R: Reader<Offset = usize>>(
    dwarf: &Dwarf<R>,
) -> gimli::Result<Vec<(Option<String>, u64, Option<u64>)>> {
    let mut results = Vec::new();
    let mut units = dwarf.units();

    while let Some(header) = units.next()? {
        let unit = dwarf.unit(header)?;
        let mut entries = unit.entries();

        // Check root DIE once (for split DWARF or general CU info)
        if let Some((_, root)) = entries.next_dfs()? {
            if let Some(attr) = root.attr_value(DW_AT_dwo_name)? {
                match attr {
                    AttributeValue::String(s) => {
                        println!(
                            "⚠️ Split DWARF detected: DW_AT_dwo_name = {}",
                            s.to_string_lossy()?
                        );
                    }
                    other => println!("⚠️ Split DWARF detected (non-string): {:?}", other),
                }
            }
        }

        // Continue traversal (starting after root DIE)
        while let Some((_, entry)) = entries.next_dfs()? {
            let mut low_pc: Option<u64> = None;
            let mut high_pc: Option<u64> = None;
            let mut name: Option<String> = None;
            let mut ranges_index: Option<u64> = None;

            // Collect all attributes first
            let mut attrs = entry.attrs();
            while let Some(attr) = attrs.next()? {
                match attr.name() {
                    DW_AT_low_pc => {
                        if let AttributeValue::Addr(a) = attr.value() {
                            low_pc = Some(a);
                        }
                    }
                    DW_AT_high_pc => match attr.value() {
                        AttributeValue::Addr(a) => high_pc = Some(a),
                        AttributeValue::Udata(off) => {
                            if let Some(lp) = low_pc {
                                high_pc = Some(lp + off);
                            }
                        }
                        _ => {}
                    },
                    DW_AT_entry_pc => {
                        if let AttributeValue::Addr(a) = attr.value() {
                            low_pc.get_or_insert(a);
                        }
                    }
                    DW_AT_ranges => {
                        if let AttributeValue::RangeListsRef(r) = attr.value() {
                            ranges_index = Some(r.0 as u64);
                        }
                    }
                    DW_AT_name => {
                        if let Ok(s) = dwarf.attr_string(&unit, attr.value()) {
                            name = Some(s.to_string_lossy()?.into_owned());
                        }
                    }
                    DW_AT_call_origin => {
                        if let AttributeValue::UnitRef(off) = attr.value() {
                            if let Ok(origin_entry) = unit.entry(off) {
                                if let Some(name_attr_val) =
                                    origin_entry.attr_value(DW_AT_name)?
                                {
                                    if let Ok(s) =
                                        dwarf.attr_string(&unit, name_attr_val)
                                    {
                                        name = Some(s.to_string_lossy()?.into_owned());
                                    }
                                }
                            }
                        }
                    }
                    DW_AT_declaration => {
                        // Declaration-only DIE — no code
                        break;
                    }
                    _ => {}
                }
            }

            // Print all address-bearing DIEs
             {
                println!("------------------------------------------------");
                println!(
                    "Tag: {:?}, name: {:?}",
                    entry.tag(),
                    name.as_deref().unwrap_or("<anon>")
                );

                // print resolved addresses
                if let Some(lp) = low_pc {
                    println!("  low_pc:  0x{lp:x}");
                }
                if let Some(hp) = high_pc {
                    println!("  high_pc: 0x{hp:x}");
                }

                if let Some(idx) = ranges_index {
                    println!("  ranges:  DebugRngListsIndex({idx})");

                    if let Some(offset) =
                        ranges_index.map(|i| gimli::RangeListsOffset(i as usize))
                    {
                        if let Ok(mut iter) = dwarf.ranges(&unit, offset) {
                            while let Ok(Some(r)) = iter.next() {
                                println!(
                                    "    range: [0x{:x} - 0x{:x})",
                                    r.begin, r.end
                                );
                            }
                        }
                    }
                }

                // print all attrs in case something interesting is hiding
                let mut attrs = entry.attrs();
                while let Some(attr) = attrs.next()? {
                    let val_str = match attr.value() {
                        AttributeValue::Addr(a) => format!("Addr(0x{a:x})"),
                        AttributeValue::Udata(u) => format!("Udata({u})"),
                        AttributeValue::Data1(d) => format!("Data1({d})"),
                        AttributeValue::Data2(d) => format!("Data2({d})"),
                        AttributeValue::Data4(d) => format!("Data4({d})"),
                        AttributeValue::Data8(d) => format!("Data8({d})"),
                        AttributeValue::String(s) => {
                            format!("String({})", s.to_string_lossy()?.into_owned())
                        }
                        other => format!("{other:?}"),
                    };
                    println!("    {:?} = {}", attr.name(), val_str);
                }

                // keep record for caller
                if let Some(lp) = low_pc {
                    results.push((name.clone(), lp, high_pc));
                }
            }
        }
    }

    Ok(results)
}
// use gimli::{
//     AttributeValue, Dwarf, Reader,
//     DW_AT_call_origin, DW_AT_call_pc, DW_AT_high_pc, DW_AT_low_pc, DW_AT_name,
//     DW_TAG_call_site, DW_TAG_inlined_subroutine,
// };

// /// Collect valid instruction start addresses (from call sites or inlined code)
// /// Returns (callee_name, start_address)
// pub fn iter_call_start<R: Reader>(dwarf: &Dwarf<R>) -> gimli::Result<Vec<(Option<String>, u64)>> {
//     let mut results = Vec::new();
//     let mut units = dwarf.units();

//     while let Some(header) = units.next()? {
//         let unit = dwarf.unit(header)?;
//         let mut entries = unit.entries();

//         while let Some((_, entry)) = entries.next_dfs()? {
//             let tag = entry.tag();
//             if tag != DW_TAG_call_site && tag != DW_TAG_inlined_subroutine {
//                 continue;
//             }

//             // Collect common fields
//             let mut callee_name: Option<String> = None;
//             let mut start_addr: Option<u64> = None;

//             let mut attrs = entry.attrs();
//             while let Some(attr) = attrs.next()? {
//                 match attr.name() {
//                     // address attributes
//                     DW_AT_call_pc | DW_AT_low_pc => {
//                         if let AttributeValue::Addr(a) = attr.value() {
//                             start_addr = Some(a);
//                         }
//                     }

//                     // optional high_pc to cross-check (not needed for now)
//                     DW_AT_high_pc => {
//                         // could be a size, but we only need start
//                     }

//                     // resolve callee reference
//                     DW_AT_call_origin => {
//                         if let AttributeValue::UnitRef(off) = attr.value() {
//                             if let Ok(origin_entry) = unit.entry(off) {
//                                 if let Some(name_attr_val) =
//                                     origin_entry.attr_value(DW_AT_name)?
//                                 {
//                                     if let Ok(s) = dwarf.attr_string(&unit, name_attr_val) {
//                                         callee_name =
//                                             Some(s.to_string_lossy()?.into_owned());
//                                     }
//                                 }
//                             }
//                         }
//                     }
//                     _ => {}
//                 }
//             }

//             if let Some(addr) = start_addr {
//                 results.push((callee_name, addr));
//             }
//         }
//     }

//     Ok(results)
// }



pub fn dump_parts(path: &Path) -> Result<(), Box<dyn Error>> {
    let a = Arena::new();
    let mut arena = FileRegistry::new(&a);
    let machine_file = arena.get_machine(path.into())?;
    let dwarf = machine_file.load_dwarf()?;

    let found = iter_function_ranges(&dwarf)?;
    if found.is_empty() {
        println!("No DW_TAG_subprogram entries with DW_AT_low_pc found in {:?}", path);
        return Ok(());
    }

    println!("Functions found in {:?}:", path);
    for (name, low, high) in found {
        match high {
            Some(hp) => println!("{:<40} 0x{:016x} - 0x{:016x}", name.unwrap_or_else(|| "<unnamed>".into()), low, hp),
            None => println!("{:<40} 0x{:016x}", name.unwrap_or_else(|| "<unnamed>".into()), low),
        }
    }

    // let found = iter_call_start(&dwarf)?;
    // if found.is_empty() {
    //     println!("No call or inlined sites found.");
    // } else {
    //     println!("Valid call/inlined ranges:");
    //     for (name, addr) in found {
    //         println!(
    //             "{:<40} 0x{:016x}",
    //             name.unwrap_or_else(|| "<unnamed>".into()),
    //             addr
    //         );
    //     }
    // }

    Ok(())
}
