use clap::Parser;
use source_viewer::args::*;
use source_viewer::subcommands::*;
use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let cli = match env::args().nth(1).as_deref().map(Cli::is_subcommand_name) {
        Some(false) => Cli {
            command: Commands::ViewSource(ViewSource::parse()),
        },
        _ => Cli::parse(),
    };

    apply_color_mode(cli.get_color());

    match cli.command {
        Commands::Walk { opts } => walk_command(opts.bin.into()),
        Commands::Sections { opts } => sections_command(opts.bins),
        Commands::Lines { opts,ignore_unknown } => lines_command(opts.bins,ignore_unknown),
        Commands::ViewSource(ViewSource {
            opts,
            all,
            walk,
            selections,
        }) => view_source_command(&opts.bin, all, walk, selections),
        Commands::ViewSources { opts } => view_sources_command(opts.bins),
        Commands::DwarfDump { opts } => dwarf_dump_command(opts.bins),
    }
}
