# Relic Web Interpreter

This is an old version of Relic that can be compiled to WASM and be run online. Everything not related to the web interpreter is removed.

> [!Warning]
> This interpreter is abandoned. I will probably NOT add any new feature to it.

## Usage

```bash
git checkout interpreter

# Build the WebAssembly module
cargo build --target wasm32-unknown-unknown --release

# Use wasm-bindgen CLI to generate the JavaScript bindings
wasm-bindgen target/wasm32-unknown-unknown/release/relic.wasm --out-dir ./pkg --target web

# You need an HTTP server to run the app
npm start # or whatever HTTP server
```

## License

MIT OR Apache 2.0
