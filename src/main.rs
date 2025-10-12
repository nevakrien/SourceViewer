use clap::Parser;
use std::error::Error;
use source_viewer::subcommands::*;
use source_viewer::args::*;

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Walk { opts }
        | Commands::ViewSource { opts, .. } => apply_color_mode(opts.color),
        Commands::Sections { opts }
        | Commands::Lines { opts }
        | Commands::ViewSources { opts }
        | Commands::DwarfDump { opts } => apply_color_mode(opts.color),
    }

    match cli.command {
        Commands::Walk { opts } => walk_command(opts.bin.into()),
        Commands::Sections { opts } => sections_command(opts.bins),
        Commands::Lines { opts } => lines_command(opts.bins),
        Commands::ViewSource { opts, all, walk,selections } =>
            view_source_command(&opts.bin, all, walk,selections),
        Commands::ViewSources { opts } => view_sources_command(opts.bins),
        Commands::DwarfDump { opts } => dwarf_dump_command(opts.bins),
    }
}
