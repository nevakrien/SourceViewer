use clap::{Arg, Command};
use std::path::PathBuf;
use std::error::Error;
use source_viewer::subcommands::{lines_command,sections_commands};



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
        );
        

    let matches = command.clone().get_matches();

    match matches.subcommand() {
        Some(("lines", sub_m)) => lines_command(sub_m),
        Some(("sections", sub_m)) => sections_commands(sub_m),
        _ => {
            command.print_help()?;
            std::process::exit(1);
        }
    }
}

