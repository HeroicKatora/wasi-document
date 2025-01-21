This stage2 loader prepares a virtual WASI environment, based on bundled data.

Building this stage2 loader requires node and esbuild. See `package.json`.

```bash
npm build
 ```

Then use it, e.g. here on the wasi-example (see `examples/wasi` for instructions).

```bash
cargo run --bin wasm-as-html -- -o target/out.html wasi-loader/out.js examples/wasi/wasi-example.wasm
```

This generates the webpage `target/out.html`.
