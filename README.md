# bitaxe-clocker
A small application which adjusts the frequency on one or serveral Bitaxes depedning on energy prices. Written in Rust.

Currently it only fetches electricity prices for Sweden, via [https://www.elprisetjustnu.se/](https://www.elprisetjustnu.se/),
but creating modules for other suppliers is possible.

## Setup

Prerequisites:

- Rust lang

An easy way to setup Rust is to use [asdf](https://asdf-vm.com/). Then run `asdf install` to get the proper version specified in the project.

## Usage
1) Create a `config.toml` from the `config.example.toml` and place it in the main diretory.

2) Build a release.

```bash
cargo build -r
```

3) Run the application.

```bash
./target/release/bitaxe-clocker
```
