use colored::Colorize;
use clap::{Arg, Command};
use std::path::PathBuf;
use std::error::Error;
use source_viewer::subcommands::*;



fn main() -> Result<(), Box<dyn Error>> {
    let mut command = Command::new("Source Viewer")
        .version("0.1")
        .author("Neva Krien")
        .about("A tool for viewing assembly and source information in binary files")
        
        .subcommand(
            Command::new("walk")
                .about("looks at the source code files next to assembly")
                .arg(
                    Arg::new("FILE") // Renamed to singular "FILE" for clarity
                        .help("Input binary/object file to process")
                        .required(true) // Ensure it is required
                        .num_args(1) // Expect exactly one argument
                        .value_parser(clap::value_parser!(PathBuf)),
                ),
        )

        .subcommand(
            Command::new("sections")
                .about("Dumps sections information for each file")
                .arg(
                    Arg::new("FILES")
                        .help("Input binary/object files to process")
                        .required(true)
                        .num_args(1..) // Allows multiple file paths
                        .value_parser(clap::value_parser!(PathBuf)),
                ),
        )

        .subcommand(
            Command::new("lines")
                .about("Annotates assembly instructions with source information")
                .arg(
                    Arg::new("FILES")
                        .help("Input binary/object files to process")
                        .required(true)
                        .num_args(1..) // Allows multiple file paths
                        .value_parser(clap::value_parser!(PathBuf)),
                ),
        )


        .subcommand(
            Command::new("view_source")
                .about("looks at the source code files that made the binary")
                .arg(
                    Arg::new("BIN")
                        .help("Input binary/object file to process")
                        .required(true)
                        .num_args(1) // Ensures only a single input file
                        .value_parser(clap::value_parser!(PathBuf)),
                )
                .arg(
                    Arg::new("all")
                        .short('a')
                        .long("all")
                        .help("Show all source files")
                        .action(clap::ArgAction::SetTrue), // Sets the flag as a binary on/off
                )
                .arg(
                    Arg::new("SELECTIONS")
                        .help("Specific indices or file paths to display")
                        .required(false)
                        .num_args(0..) // Allows zero or more additional arguments
                        .value_parser(clap::builder::ValueParser::new(|s: &str| {
                            if let Ok(index) = s.parse::<usize>() {
                                Ok(FileSelection::Index(index))
                            } else {
                                let path = std::fs::canonicalize(PathBuf::from(s))
                                .map_err(|e| format!("Error canonicalizing path {}: {}", s, e))?;
                                if path.exists() {
                                    Ok(FileSelection::Path(path))
                                } else {
                                    Err(format!("'{}' is not a valid index or existing path", s))
                                }
                            }
                        }))
                ),

            )


        .subcommand(
            {
            let description = format!(
                "{}: {}",
                "Not Finished".red(),
                "dumps the dwarf debug information in the files"
            );
            Command::new("dwarf_dump")
                .about(description)
                .arg(
                    Arg::new("FILES")
                        .help("Input binary/object files to process")
                        .required(true)
                        .num_args(1..) // Allows multiple file paths
                        .value_parser(clap::value_parser!(PathBuf)),
                )
            },
        );
        

    let matches = command.clone().get_matches();

    match matches.subcommand() {
        Some(("lines", sub_m)) => lines_command(sub_m),
        Some(("sections", sub_m)) => sections_command(sub_m),
        Some(("view_source", sub_m)) => view_source_command(sub_m),
        Some(("dwarf_dump", sub_m)) => dwarf_dump_command(sub_m),
        Some(("walk", sub_m)) => walk_command(sub_m),
        _ => {
            command.print_help()?;
            std::process::exit(1);
        }
    }
}

