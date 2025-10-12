use std::panic;
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

    panic::set_hook(Box::new(|info| {
        // Try to detect a broken pipe panic
        // we would of liked a specific downcast but no luck
        // rusts defualt print errors with a str for some supid reason
        let is_broken_pipe = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.contains("Broken pipe"))
            .unwrap_or_else(|| {
                info.payload()
                    .downcast_ref::<String>()
                    .map(|s| s.contains("Broken pipe"))
                    .unwrap_or(false)
            });

        if is_broken_pipe {
            // print a short friendly message instead of a backtrace
            eprintln!("⚠️  Broken pipe: the output stream (stdout/stderr) closed early.");
            std::process::exit(0);
        }

        // Default behavior for all other panics
        eprintln!("\n[panic] {}", info);
    }));

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
