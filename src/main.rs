use source_viewer::skiper::dump_parts;
use source_viewer::errors::downcast_chain_ref;
use source_viewer::errors::PrintError;
use std::process::ExitCode;
use clap::Parser;
use source_viewer::args::*;
use source_viewer::subcommands::*;
use std::env;


fn main() -> ExitCode {
    let cli = match env::args().nth(1).as_deref()
    .map(|s| s.is_empty() | s.starts_with("-") | Cli::is_subcommand_name(s)) {
        Some(false) => {
            eprintln!("assuming view_source"); 
            Cli {
                command: Commands::ViewSource(ViewSource::parse()),
            }
        },
        _ => Cli::parse(),
    };

    apply_color_mode(cli.get_color());


    let res = match cli.command {
        Commands::Walk { opts } => walk_command(opts.bin.into()),
        Commands::Sections { opts } => sections_command(opts.bins),
        Commands::Lines { opts, ignore_unknown } => lines_command(opts.bins, ignore_unknown),
        Commands::ViewSource(ViewSource { opts, all, walk, selections }) =>
            view_source_command(&opts.bin, all, walk, selections),
        Commands::ViewSources { opts } => view_sources_command(opts.bins),
        Commands::DwarfDump { opts } => dwarf_dump_command(opts.bins),
        
        Commands::DumpParts {opts } => {dump_parts(&opts.bin)},
    };

    ExitCode::from(match res {
        Ok(_) => 0,
        Err(e) => {
             if let Some(print_err) = downcast_chain_ref::<PrintError>(&*e){
                if print_err.0.kind()==std::io::ErrorKind::BrokenPipe { 
                    eprintln!("⚠️  Broken pipe: the output stream (stdout/stderr) closed early."); 
                    return ExitCode::from(1);
                }
             }
             
             eprintln!("{e}");
             1
        }
    })
}
