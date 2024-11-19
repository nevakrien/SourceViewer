# SourceViewer
assembly viewing tool

the goal is to allow viewing dissasmblies from the perspective of the source file without taking over your compilation setup.

a typical workload would look something like 
```bash
	SourceViewer view_source sample_code/build/linux_x86_64

	Source files:
	0: "/snap/zig/11625/lib/libc/glibc/csu/elf-init-2.33.c"
	1: "/home/user/Desktop/rust_stuff/SourceViewer/sample_code/get_time.c"
	2: "/snap/zig/11625/lib/libc/glibc/sysdeps/x86_64/crtn.S"
	3: "/snap/zig/11625/lib/libc/glibc/sysdeps/x86_64/crti.S"
	4: "/snap/zig/11625/lib/libc/glibc/sysdeps/x86_64/start-2.33.S"

	SourceViewer view_source sample_code/build/linux_x86_64 -w 0

```
here we looked at the files that composed our binary and then went into the first file to view its contributions


# TODO 
1. symbol resolution: find the jump instructions and switch pointers with the names of the target
2. interactive view

# Tests
a very good test case is runing 
```bash
cargo run walk target/debug/SourceViewer
```

is a very good way to see where we are at. if you are anoyyed by the delay on the load then using release mode should solve most the issue.

# Issues
currently we cant run detail=true because that removes some instructions from the end...
however without it we cant check for whether or not an instruction is a jump.
kind of puts us in a tricky position we will see what to do here

# Platforms
its becoming ridiclously hard to get correct dwarf info on windows and mac files.
for the life of me I cant bother. so for now i am building only for linux but the parsing on other platforms is there.
if you manage to get a valid dwarf format on windows or mac the program should support it

