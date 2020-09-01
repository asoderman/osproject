#!/bin/bash

# Opens the debugger (gdb) once `cargo xrun -- -S` is invoked
# -- loads kernel symbols and sets a breakpoint at kernel_main

gdb \
    -ex "target remote localhost:1234" \
    -ex "file ./target/x86_64-target/debug/rustos" \
    -ex "break kernel_main" \
    -ex "continue"
