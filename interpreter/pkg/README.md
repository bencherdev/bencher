<div align="center">
  <h1>TableFlow Interpreter</h1>
  <strong>The interpreter for TableFlow, written in Rust compiled to WebAssembly.</strong>
</div>

# Build

## Build `wasm-pack build`

```
wasm-pack build
```

## Test in Headless Browsers with `wasm-pack test`

```
wasm-pack test --headless --firefox
```

## Publish to NPM with `wasm-pack publish`

```
wasm-pack publish
```

# Tooling

- [`wasm-bindgen`](https://github.com/rustwasm/wasm-bindgen) for communicating
  between WebAssembly and JavaScript.
- [`console_error_panic_hook`](https://github.com/rustwasm/console_error_panic_hook)
  for logging panic messages to the developer console.
- [`wee_alloc`](https://github.com/rustwasm/wee_alloc), an allocator optimized
  for small code size.

# License

The LICENSE (MIT) applies to all files in this repository,
except for those under any directory named "enterprise",
which are covered by the TableFlow Enterprise license.
The TableFlow Enterprise license may be included within
these directories
