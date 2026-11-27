# Windows x64 build and validation

Windows support is being validated. A successful source build alone does not certify the installer or every tool. macOS releases remain available independently.

## Offline package

The NSIS installer targets Windows 11 x64. It includes MuPDF, the native reader and print workers, qpdf, Ghostscript, Tesseract, Chinese/English OCR models, LibreOffice and its fonts. The WebView2 offline installer is embedded; no document-processing component is downloaded at application runtime. The application installs for the current user, with English and Simplified Chinese installer languages.

The Windows runtime uses MSYS2 UCRT64 native binaries and their transitive PE DLL imports, without shipping the MSYS POSIX shell or build tools. Pacman verifies package signatures; the runtime records package versions, archive hashes, licenses and installed-file hashes. LibreOffice 26.8.3 and Ghostscript resource sources have explicit SHA-256 checks. Runtime builds currently resolve MSYS2 packages from its repository; the recorded receipts describe each actual build, but are not yet a frozen dependency lock for future rebuilds.

## Rebuild

Use a Windows x64 machine with Node.js 22, stable Rust MSVC, Visual Studio C++ build tools, Python 3.11+ and MSYS2 UCRT64. Install the MSYS2 packages listed in `.github/workflows/windows.yml`, then run:

```powershell
python scripts/prepare-windows.py --msys C:/msys64
npm ci
npm run check
npm run test:ui
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo test --locked --manifest-path src-tauri/Cargo.toml -- --ignored
cargo build --locked --manifest-path src-tauri/Cargo.toml --bin bpdf-cli
npx tauri build --config src-tauri/tauri.windows.conf.json --bundles nsis
python -m pip install psutil==7.0.0
python scripts/test-windows-install.py
```

Python, MSYS2 and Node.js are build/test tools, not end-user requirements. `test-windows-install.py` installs the freshly built package into a test directory under LOCALAPPDATA; run it on a disposable validation machine. Outputs are under `src-tauri/target/release/bundle/nsis` and evidence under `work/windows`.

## Validation boundaries

The workflow verifies installed resource hashes, PDF/Office integration samples, Unicode paths, basic 1,000-page reading, cancellation recovery, and a roughly 200 MB / 1,000-page rasterization workload with sampled process-tree memory. Test process PATH excludes MSYS2/build tools. It uses Windows Server 2022, not a Windows 11 desktop, and the runner already has system dependencies; this does not establish a clean offline Windows 11 installation or high-refresh GUI performance. Printer output and two-device macOS/Windows LAN collaboration still require hardware validation.

Windows workers are started suspended, assigned to a kill-on-close Job Object, then resumed. A 1.5 GiB job memory limit bounds each worker tree. Cancellation, timeout and parent exit terminate descendants; this is process/resource containment, not the macOS filesystem/network sandbox. Equivalent Windows document-worker file/network isolation remains outstanding. Macros are disabled in the private Office profile and document JavaScript is not executed by MuPDF.

Installed application bytes, installer bytes and sampled peak worker memory are reported separately. WebView2's additional system installation footprint is not included in application-directory size and must be measured on a clean machine before final acceptance.

Implementation references: [Tauri offline Windows installers](https://tauri.app/distribute/windows-installer/), [LibreOffice Windows command-line entry point](https://help.libreoffice.org/latest/en-GB/text/shared/guide/start_parameters.html?DbPAR=WRITER&System=WIN), [MSYS2 UCRT64 environments](https://www.msys2.org/docs/environments/).
