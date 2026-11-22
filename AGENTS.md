# Working on Flik

Flik is a Linux tool for writing slide decks in Markdown and presenting them in the browser. Markdown is the source of truth; saving the deck or its local images refreshes the preview.

**Stack:** Rust (2024 edition), Clap for the CLI, pulldown-cmark for Markdown, Serde for YAML front matter and JSON slide data, and tiny_http for the loopback server. The UI uses HTML, CSS, and vanilla JavaScript with no frontend build step.

## Code map

- `src/main.rs` handles Clap options and browser launch. `src/preview/` serves the loopback preview and resolves local images.
- `src/lib.rs`, `src/slide.rs`, and `src/theme.rs` parse decks and render slides. Keep this library independent of HTTP and local filesystem access so a desktop host can reuse it.
- `web/app.js` handles preview updates, navigation, and fullscreen. `web/renderer.js` and `web/style.css` display the slide model.

Run `cargo fmt --check`, `cargo test --locked`, and `cargo clippy --locked --all-targets -- -D warnings`. To smoke-test the UI, run `cargo run --locked -- examples/demo.md`.

Keep `README.md` short. Document CLI and deck syntax changes in `docs/guide.md` and update `examples/demo.md` when appropriate.
