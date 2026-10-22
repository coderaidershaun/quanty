# quanty browser app

Check the browser target from the workspace root:

```sh
cargo clippy -p web-ui --target wasm32-unknown-unknown -- -D warnings
```

Build the bundle from this folder:

```sh
trunk build --release
```

The server embeds `dist/` when it is built. Build the bundle before building `quanty`.
The app uses WebGPU with wgpu's WebGL fallback, same-origin cookies, and no service worker.
