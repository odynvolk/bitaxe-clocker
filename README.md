# bitaxe-clocker
A small application which adjusts the frequency on one or serveral Bitaxes depedning on energy prices. Written in Rust.

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
