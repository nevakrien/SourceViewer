# SourceViewer
assembly viewing tool

the goal is to allow viewing dissasmblies from the perspective of the source file without taking over your compilation setup.

# TODO 
1. symbol resolution: find the jump instructions and switch pointers with the names of the target
2. interactive view

# Issues
currently we cant run detail=true because that removes some instructions from the end...
however without it we cant check for whether or not an instruction is a jump.
kind of puts us in a tricky position we will see what to do here

# Platforms
its becoming ridiclously hard to get correct dwarf info on windows and mac files.
for the life of me I cant bother. so for now i am building only for linux but the parsing on other platforms is there.
if you manage to get a valid dwarf format on windows or mac the program should support it

