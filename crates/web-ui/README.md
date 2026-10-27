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

The browser's back button, the phone back gesture and Alt+Left go back inside the app while it can, then leave the page as before.
Cmd/Ctrl+`[` and `]` turn the source page, so the browser does not also go back or forward.
The canvas stays inside a phone's safe area, so the notch and the home bar cover nothing; the page itself does not zoom.
The app opens dark. The person's light or dark choice is kept in the browser's local storage, and the page colour around the canvas follows it.
