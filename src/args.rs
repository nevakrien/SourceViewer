use clap::CommandFactory;
use clap::builder::ValueParser;
use clap::{Parser, Subcommand, ValueEnum};
use colored::{Colorize, control::SHOULD_COLORIZE};
use std::path::PathBuf;

/// When colorized output should be shown
#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum ColorMode {
    /// Always use color, even when not writing to a terminal
    Always,
    /// Automatically detect when to use color (default)
    Auto,
    /// Never use color
    Never,
}

pub fn apply_color_mode(mode: ColorMode) {
    match mode {
        ColorMode::Always => SHOULD_COLORIZE.set_override(true),
        ColorMode::Never  => SHOULD_COLORIZE.set_override(false),
        ColorMode::Auto   => {} // let `colored` auto-detect
    }
}

/// Shared options for commands that take **one** binary
#[derive(Parser, Debug, Clone)]
pub struct SingleBinOpts {
    #[arg(
        long,
        value_enum,
        num_args(0..=1),
        default_missing_value = "always",
        default_value_t = ColorMode::Auto,
        global = true,
        help = "Colorize output: always, auto, never (default: auto). \
                Using --color without a value implies 'always'."
    )]
    pub color: ColorMode,

    #[arg(value_name = "BIN", required = true,
          help = "Input binary/object file to process")]
    pub bin: PathBuf,
}

/// Shared options for commands that take **multiple** binaries
#[derive(Parser, Debug, Clone)]
pub struct MultiBinOpts {
    #[arg(
        long,
        value_enum,
        num_args(0..=1),
        default_missing_value = "always",
        default_value_t = ColorMode::Auto,
        global = true,
        help = "Colorize output: always, auto, never (default: auto). \
                Using --color without a value implies 'always'."
    )]
    pub color: ColorMode,

    #[arg(value_name = "BINS", required = true, num_args(1..),
          help = "Input binary/object files to process")]
    pub bins: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum FileSelection {
    Index(usize),
    Path(PathBuf),
}

fn file_selection_parser() -> ValueParser {
    ValueParser::new(|s: &str| -> Result<FileSelection, String> {
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
    })
}

#[derive(Parser, Debug, Clone)]
pub struct ViewSource {
    #[command(flatten)]
    pub opts: SingleBinOpts,

    #[arg(short, long, help = "Show all source files")]
    pub all: bool,

    #[arg(short, long, help = "Start the walk command on the selected file")]
    pub walk: bool,

    #[arg(
        value_name = "SELECTIONS",
        num_args(0..),
        value_parser = file_selection_parser(),
        help = "Specific indices or file paths to display"
    )]
    pub selections: Vec<FileSelection>,
}

/// Top-level CLI
#[derive(Parser, Debug)]
#[command(
    author = "Neva Krien",
    version = env!("CARGO_PKG_VERSION"),
    about = "A tool for viewing assembly and source information in binary files"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

}

impl Cli {
    pub fn get_color(&self) -> ColorMode {
       self.command.get_color()
    }

    pub fn is_subcommand_name(name: &str) -> bool {
        let cmd = Self::command();

        let x = cmd.get_subcommands().any(|sc| {
            sc.get_name() == name || sc.get_all_aliases().any(|a| a == name)
        }); x
    }
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Looks at the source code files next to assembly")]
    Walk {
        #[command(flatten)]
        opts: SingleBinOpts,
    },

    #[command(about = "Dumps sections information for each file")]
    Sections {
        #[command(flatten)]
        opts: MultiBinOpts,
    },

    #[command(about = "Annotates assembly instructions with source information")]
    Lines {
        #[command(flatten)]
        opts: MultiBinOpts,
    },

    #[command(

        about = "Looks at the source code files that made the binary",
        visible_aliases = ["view_source",""]
    )]
    ViewSource(ViewSource),

    #[command(
        about = "Dumps all source files used to make binaries",
        visible_aliases = ["view_sources"]
    )]
    ViewSources {
        #[command(flatten)]
        opts: MultiBinOpts,
    },

    #[command(
        about = format!(
            "{}: {}",
            "Not Finished".red(),
            "Dumps the DWARF debug information in the files"
        ),
        visible_aliases = ["dwarf_dump"]
    )]
    DwarfDump {
        #[command(flatten)]
        opts: MultiBinOpts,
    },
}

impl Commands {
    pub fn get_color(&self)->ColorMode{
        match self{
            Commands::Walk { opts }
            | Commands::ViewSource(ViewSource{ opts, .. }) => opts.color,
            Commands::Sections { opts }
            | Commands::Lines { opts }
            | Commands::ViewSources { opts }
            | Commands::DwarfDump { opts } => opts.color
        }
    }
}