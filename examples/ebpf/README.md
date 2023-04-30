# ebpf

## Prerequisites

1. Install a rust stable toolchain: `rustup install stable`
1. Install a rust nightly toolchain with the rust-src component: `rustup toolchain install nightly --component rust-src`
1. Install LLVM 15: `sudo ./llvm.sh 15 && sudo apt install libpolly-15-dev libz-dev`
1. Install bpf-linker: `cargo install --no-default-features --features system-llvm --locked bpf-linker@0.9.5`

## Build eBPF

```bash
cargo xtask build-ebpf
```

To perform a release build you can use the `--release` flag.
You may also change the target architecture with the `--target` flag.

## Build Userspace

```bash
cargo build
```

## Run

```bash
RUST_LOG=info cargo xtask run
```

## Build Release

```bash
RUST_LOG=info cargo xtask build-ebpf --release
```

## Run Benchmarks

```bash
cargo +nightly bench
```
