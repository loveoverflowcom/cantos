# Bounded WASM formatter probe

Build from the repository root with the existing toolchain and a reused target directory:

```sh
CARGO_TARGET_DIR=/path/to/existing-target CARGO_INCREMENTAL=0 cargo build --locked --target wasm32-unknown-unknown --manifest-path packages/localization/qualification/wasm/Cargo.toml
wasm-bindgen --target nodejs --out-dir packages/localization/qualification/wasm/generated /path/to/existing-target/wasm32-unknown-unknown/debug/cantos_localization_wasm_qualification.wasm
node packages/localization/qualification/wasm/verify.cjs
```

Use wasm-bindgen **0.2.100**, matching the lock. It is already Trunk's cached binding tool
on the qualifying Mac. No toolchain download or second target directory is required.
The 15 exact vectors call the real shared formatter compiled to WASM. This probe has no DOM,
Leptos lifecycle, browser storage, Kotlin binding, Android or iOS observation. The currency
code boundary deliberately rejects unsupported codes before invoking the closed USD/VND API.
