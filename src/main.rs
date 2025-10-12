use std::sync::atomic::Ordering;
use std::sync::atomic::AtomicU8;
use std::process::ExitCode;
use std::panic;
use clap::Parser;
use source_viewer::args::*;
use source_viewer::subcommands::*;
use std::env;

static ERR_EXIT_CODE : AtomicU8 = AtomicU8::new(1);

fn main() -> ExitCode {
    let cli = match env::args().nth(1).as_deref()
    .map(|s| s.is_empty() | s.starts_with("-") | Cli::is_subcommand_name(s)) {
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
            ERR_EXIT_CODE.store(0,Ordering::Release);
            return;
        }

        // Default behavior for all other panics
        eprintln!("\n[panic] {}", info);
    }));

    let res = panic::catch_unwind(|| {
        match cli.command {
            Commands::Walk { opts } => walk_command(opts.bin.into()),
            Commands::Sections { opts } => sections_command(opts.bins),
            Commands::Lines { opts, ignore_unknown } => lines_command(opts.bins, ignore_unknown),
            Commands::ViewSource(ViewSource { opts, all, walk, selections }) =>
                view_source_command(&opts.bin, all, walk, selections),
            Commands::ViewSources { opts } => view_sources_command(opts.bins),
            Commands::DwarfDump { opts } => dwarf_dump_command(opts.bins),
        }
    });

    ExitCode::from(match res {
        Ok(Ok(_)) => 0,
        Ok(Err(e)) => {
             eprintln!("{e}");
             ERR_EXIT_CODE.load(Ordering::Acquire)
        }
        _=> ERR_EXIT_CODE.load(Ordering::Acquire),
    })
}
