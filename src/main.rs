use clap::CommandFactory;
use clap::Parser;
use std::error::Error;
use source_viewer::subcommands::*;
use source_viewer::args::*;

fn main() -> Result<(), Box<dyn Error>> {
    //normal parse
    let cli = Cli::parse();

    apply_color_mode(cli.get_color());

    match cli.command {
        // normal subcommands
        Some(command)=> match command {
            Commands::Walk { opts } => walk_command(opts.bin.into()),
            Commands::Sections { opts } => sections_command(opts.bins),
            Commands::Lines { opts } => lines_command(opts.bins),
            Commands::ViewSource { opts, all, walk, selections } =>
                view_source_command(&opts.bin, all, walk, selections),
            Commands::ViewSources { opts } => view_sources_command(opts.bins),
            Commands::DwarfDump { opts } => dwarf_dump_command(opts.bins),
        },

        // fallback: no subcommand, user just passed a binary path
        None=>{
            if cli.bins.is_empty() {
                Cli::command().print_help()?;
                std::process::exit(1);
            }
            view_sources_command(cli.bins)
        },
    }

}
