# SourceViewer
assembly viewing tool (this was a pet project but i found myself actually reaching for it so here i am publishing)
the goal is to allow viewing dissasmblies from the perspective of the source file without taking over your compilation setup.


![example of walk](https://github.com/nevakrien/SourceViewer/raw/main/example_cpp.png)

## Installation

You can install **SourceViewer** using **Cargo** (recommended) or download prebuilt binaries from the [Releases page](https://github.com/nevakrien/SourceViewer/releases).

### Using Cargo
```bash
cargo install source_viewer
```
or

```bash
cargo binstall source_viewer
```


# Quick Guide
a typical workload would look something like 
```bash
	SourceViewer sample_code/build/linux_x86_64
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
	SourceViewer sample_code/build/linux_x86_64 -w 0
```
here we looked at the files that composed our binary and then went into the first file to view its contributions.


there are also useful subcommands like "lines" which shows the entire assembly file,
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


# Tests
a very good test case is runing 
```bash
cargo run walk target/debug/SourceViewer
```

# Platforms
this is mostly supported for unix and specifically linux/bsd for 2 reasons:
1. dwarf on windows/apple is very anoying
2. windows terminals are fairly anoying

we do ship to all platforms but as of now there are issues

