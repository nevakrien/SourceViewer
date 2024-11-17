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
            Command::new("view_source")
                .about("looks at the source code files")
                .arg(
                    Arg::new("FILES")
                        .help("Input binary/object files to process")
                        .required(true)
                        .num_args(1..) // Allows multiple file paths
                        .value_parser(clap::value_parser!(PathBuf)),
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

