# JVankoM
A minimal, single-threaded JVM written in Rust for executing Java 6-8 bytecode.

## Features

- Supports branching, loops, classes, fields, methods, exceptions, inheritance, and interfaces
- Runs main method entry point
- Printing with System.out
- Caching for improved performance

## Limitations

- Single-threaded
- Partial JVM spec implementation
- No file I/O, networking, or full library support such as reflection
- Most real Java applications will probably not run correctly

## Build

Build using `cargo build` for a debug build or `cargo build -r` for a release build. Run tests with `cargo test`. Requires the Java 8 runtime library **rt.jar** and the custom library **jvankomrt.jar**.
