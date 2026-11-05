# Better PDF

**English** · [简体中文](README.zh-CN.md)

Make PDF better. A lightweight, local-first PDF workspace.

**[Download from Releases](https://github.com/Starry331/BetterPDF/releases)** · [Current development preview](https://github.com/Starry331/BetterPDF/releases/tag/v0.1.0-dev.2)

The current DMG requires **macOS 27.2+ on Apple Silicon** and is not Developer ID signed or notarized. macOS 14 compatibility and the Windows installer are still in development.

## Features and status

Read, edit, annotate, convert, and organize PDFs locally. Built with Tauri 2, Svelte 5, TypeScript, Rust, and native MuPDF rendering.

**This is a development preview, not the complete first release.** All 34 tools have processing pipelines and locally verified output samples. This does not establish compatibility with every format, layout, damaged document, or target platform. See [validation status](docs/STATUS.md) for current coverage and limitations.

## Development

Install Node.js, Rust, and Xcode command line tools on macOS or MSVC build tools on Windows. From the project directory:

```sh
npm ci
npm run check
cargo check --manifest-path src-tauri/Cargo.toml
npm run desktop
```

Engine components must be staged in `src-tauri/resources/runtime`. The application does not download missing components or use separately installed conversion software. Runtime binaries are excluded from the source repository; the current development bundle uses macOS arm64 components.

The macOS assembly script requires MuPDF, qpdf, Ghostscript, Tesseract, and a standalone headless LibreOffice build. Set `BPDF_OFFICE_SOURCE` to the LibreOffice `.app` path, then run `npm run prepare:runtime`. Dependency builds are not yet reproducible with pinned source checksums, so this script is not a production release supply chain.

```sh
cargo build --manifest-path src-tauri/Cargo.toml --bin bpdf-cli
npm run test:engine
npm test
npm run bundle:dev
```

`bundle:dev` creates a local development `.app`, not a signed or notarized release. `npm run bundle` first checks release evidence. macOS 14 compatibility requires auditing every Mach-O binary, not just changing the application's declared minimum version:

```sh
python3 scripts/audit-macos.py src-tauri/target/release/bundle/macos/BPDF.app --report work/app-audit.json --strict
```

## Project structure

- `src/`: reading workspace, tool options, task queue, and editing panels.
- `src-tauri/src/documents.rs`: document snapshots, undo/redo, saving, and render cache.
- `src-tauri/src/engine.rs`: private request files, process isolation, cancellation, and timeouts.
- `src-tauri/src/jobs.rs`: processing pipelines for the 34 tools.
- `src-tauri/src/export.rs`: shared layout model to DOCX, PPTX, XLSX, EPUB, and Markdown.
- `src-tauri/src/caj.rs`: bounded CAJ/HN parsing.
- `native/reader.c`: MuPDF C rendering interface and worker process.
- `src-tauri/resources/engine/worker.js`: trusted MuJS processing script; document JavaScript is not executed.
- `tests/`: file generation, conversion, and output reopening checks.

## Working with documents

Changes are saved as new files; existing output paths are never overwritten. Passwords reach the engine through private temporary files rather than command-line arguments. Conversion and OCR use a single heavy-task queue. Documents stay local unless you explicitly share a copy with invited LAN peers.

Text editing removes the original content before writing the replacement. Image resources are copied per page so moving an image does not modify other pages. Check converted output, especially complex tables, reading order, substituted fonts, and CAJ documents. Tools report conversion warnings and reject unknown formats. Printing currently opens the system PDF viewer.

## LAN collaboration — experimental

Use **Share** in the sidebar to host a copy of the current document or join with an invitation. Select the host's local IPv4 address; both devices need BPDF with its complete runtime. Connections use TLS and invitation certificate verification. The host commits edits and reports conflicts explicitly.

macOS and Windows share the Rust networking implementation, but Windows device and installer validation remain pending. See the [status report](docs/STATUS.md) for limitations.

```sh
cargo test --manifest-path src-tauri/Cargo.toml lan_roundtrip -- --ignored
python3 scripts/stress-check.py
```

## License

Project code is licensed under [AGPL-3.0-only](LICENSE). Third-party components retain their own licenses. Collected license files are included in the development runtime; a production release still requires complete corresponding sources, patches, build materials, and font/model license inventories. See [third-party components](docs/THIRD_PARTY.md).
