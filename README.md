# SourceViewer
Assembly viewing tool with the goal of allowing viewing disassemblies from the perspective of the source file without taking over your compilation setup, any profiling/debug build would do.

SourceViewer lazy loads dwarf debug information to facilitate this.

![example of walk](https://github.com/nevakrien/SourceViewer/raw/v0.4.0/example_cpp.png)

This tool is intended for use with C/C++/Rust and is tested on these languges.

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
Gere we looked at the files that composed our binary and then went into the first file to view its contributions. While in the walk menu pressing h would render a popup with the controls.

the walk menu exposes 2 main ways to interact with assembly
1. using Enter the selected source line is expanded/collapsed on the asm view
2. using Space the selected asm location is expanded/collapsed

so for example if you want to verify that a function call in line 100 of small.cpp was properly inlined you would start with

```bash
SourceViewer libsmall.so src/small.cpp -w 100 #using shorthand view_source is implied
```

then in walk click Enter and view the instructions.


sometimes you would want to view a binary directly rather than being tighed to a specific source file. this is expecially useful for smaller programs.

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

sections an alternative to lines which shows ALL sections and a littele less detail
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

# Limitations
1. SourceViewer is specifically designed to be very quick to open even on larger files.
as such sometimes errors are discovered late.

2. also note that on assembly that is not mapped to source is not guaranteed to be correct.
this is because sometimes compilers would leave data directly in a code section. and there is absolutely no way to detect that.

However most ISAs are specifically designed with this in mind so errors should not go out of control.

3. at the moment we only support dwarf 



# Tests
a very good test case is runing 
```bash
cargo run walk target/debug/SourceViewer
```

compare_test.sh and diff_test.sh are a way to check aginst regressions.
they work by using the installed version of SourceViewer and comparing it to the build version

# Platforms
this is mostly supported for unix and specifically linux/bsd because dwarf is the main format.
we might extend in the future.

