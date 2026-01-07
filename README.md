# SourceViewer
Assembly viewing tool with the goal of allowing viewing disassemblies from the perspective of the source file without taking over your compilation setup, any profiling/debug build would do.

SourceViewer lazy loads dwarf debug information to facilitate this even for larger projects/libraries.

![example of walk](https://github.com/nevakrien/SourceViewer/raw/v0.4.0/example_cpp.png)

This tool is intended for use with C/C++/Rust and is tested on these languages.

## Installation

You can install **SourceViewer** using **Cargo** (recommended) or download prebuilt binaries from the [Releases page](https://github.com/nevakrien/SourceViewer/releases).

Building from source is also an option but note that the code in the repo is still very unstable.

### Using Cargo
```bash
cargo install source_viewer --locked
```
or

```bash
cargo binstall source_viewer
```


# Quick Guide
A typical workload would look something like 
```bash
	SourceViewer view_source sample_code/build/linux_x86_64
```
```
	Source files:
	0: "/snap/zig/11625/lib/libc/glibc/csu/elf-init-2.33.c"
	1: "/home/user/Desktop/rust_stuff/SourceViewer/sample_code/get_time.c"
	2: "/snap/zig/11625/lib/libc/glibc/sysdeps/x86_64/crtn.S"
	3: "/snap/zig/11625/lib/libc/glibc/sysdeps/x86_64/crti.S"
	4: "/snap/zig/11625/lib/libc/glibc/sysdeps/x86_64/start-2.33.S"
```
```bash
	SourceViewer view_source sample_code/build/linux_x86_64 -w 0
```
Here we looked at the files that composed our binary and then went into the first file to view its contributions. While in the walk menu pressing h would render a popup with the controls.

the walk menu exposes 2 main ways to interact with assembly
1. using Enter the selected source line is expanded/collapsed on the asm view
2. using Space the selected asm location is expanded/collapsed

so for example if you want to verify that a function call in line 100 of small.cpp was properly inlined you would start with

```bash
SourceViewer libsmall.so src/small.cpp -w 100 #using shorthand view_source is implied
```

then in walk click Enter and view the instructions.


sometimes you would want to view a binary directly rather than being tied to a specific source file. this is especially useful for smaller programs.

there are useful subcommands like "lines" which shows the entire assembly file,
```bash
	SourceViewer lines sample_code/build/linux_x86_64
```
```
	Loading file "sample_code/build/linux_x86_64"
	.text
	0x010134d0: xor    ebp, ebp        <unknown> /snap/zig/14333/lib/libc/glibc/sysdeps/x86_64/start-2.33.S:63
	0x010134d2: mov    r9, rdx         <unknown> /snap/zig/14333/lib/libc/glibc/sysdeps/x86_64/start-2.33.S:79
	0x010134d5: pop    rsi             <unknown> /snap/zig/14333/lib/libc/glibc/sysdeps/x86_64/start-2.33.S:85
	0x010134d6: mov    rdx, rsp        <unknown> /snap/zig/14333/lib/libc/glibc/sysdeps/x86_64/start-2.33.S:88
	0x010134d9: and    rsp, 0xfffffffffffffff0 <unknown> /snap/zig/14333/lib/libc/glibc/sysdeps/x86_64/start-2.33.S:90
	...
```

it can be pumped nicely into less like so
```bash
	SourceViewer lines sample_code/build/linux_x86_64 --color | less -r
```
which is likely the main way you would want to use it.

sections an alternative to lines which shows ALL sections and a little less detail
```bash
	SourceViewer sections sample_code/build/linux_x86_64 --color | less -r
```
```
Loading file "sample_code/build/linux_x86_64"
...
Non-Executable Section: .eh_frame_hdr (5004 bytes)
Non-Executable Section: .eh_frame (24444 bytes)
Code Section: .text (316986 bytes)
  0x010134d0: xor    ebp, ebp                       
  0x010134d2: mov    r9, rdx                        
  0x010134d5: pop    rsi                            
  0x010134d6: mov    rdx, rsp                       
...
```
and again it can be pumped into less.

```bash
	SourceViewer sections sample_code/build/linux_x86_64 --color | less -r
```

most subcommands are intended for use with other tools. for example `functions` is extremely useful when combined with grep

```bash
SourceViewer functions sample_code/llvm-impl/libsmall_lang.so  | grep LLVM
```

is a quick way to find all LLVM functions in a project.

# Configuration
SourceViewer can be configured by writing to files at the system level.
the config-paths command shows the file paths we would use on your system. if the files don't exist SourceViewer would use the default behavior.

The walk configuration file should be located at `~/.config/source-viewer/walk-config.toml` and supports the following options:

```toml
# Layout Configuration
asm_percent = 53  # Percentage of screen height for assembly view (0-100, default: 53)

# Performance Configuration  
frames_per_second = 30  # Frame rate for terminal updates (default: 30)
fps = 30               # Alternative alias for frames_per_second

# Display Configuration
show_line_numbers = true  # Whether to show line numbers by default (default: true)
line_numbers = true       # Alternative alias for show_line_numbers
```

## Available Configuration Options

### `asm_percent`
- **Type**: Integer (0-100)
- **Default**: 53
- **Description**: Controls the vertical split between source code and assembly view. A value of 53 means 53% of the screen height is used for the assembly view and 47% for the source code view.

### `frames_per_second` (alias: `fps`)
- **Type**: Integer (1-120)
- **Default**: 30
- **Description**: Controls the frame rate for terminal updates during walk mode. Higher values provide smoother scrolling but may use more CPU. Lower values reduce CPU usage but may feel less responsive.
- **Aliases**: `fps` (shorter alternative)

### `show_line_numbers` (alias: `line_numbers`)
- **Type**: Boolean
- **Default**: true
- **Description**: Controls whether line numbers are displayed by default in the walk interface.
- **Aliases**: `line_numbers` (more intuitive alternative)

## Configuration Examples

### High Performance Setup
```toml
# For smoother scrolling on powerful systems
asm_percent = 60
fps = 60           # Use short alias
line_numbers = true
```

### Low Resource Setup
```toml
# For slower systems or remote connections
asm_percent = 50
frames_per_second = 15  # Use full name
show_line_numbers = false
```

### Gaming/Optimized Setup
```toml
# Maximum performance for fast navigation
asm_percent = 70
fps = 120
line_numbers = false
```

# Uninstall
just deleting the executable should be enough. if you made config files manually you can delete them.

# Limitations
1. SourceViewer is specifically designed to be very quick to open even on larger files.
as such sometimes errors are discovered late.

2. also note that on assembly that is not mapped to source is not guaranteed to be correct.
this is because sometimes compilers would leave data directly in a code section. and there is absolutely no way to detect that.

However most ISAs are specifically designed with this in mind so errors should not go out of control.

3. at the moment we only support dwarf 



# Tests
a very good test case is running 
```bash
cargo run walk target/debug/SourceViewer
```

compare_test.sh and diff_test.sh are a way to check against regressions.
they work by using the installed version of SourceViewer and comparing it to the build version

# Platforms
this is mostly supported for unix and specifically linux/bsd because dwarf is the main format.
we might extend in the future.
