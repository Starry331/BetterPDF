# Better PDF

本地 PDF 阅读、编辑与转换桌面应用。Tauri 2 / Svelte 5 / TypeScript / Rust，原生 MuPDF 渲染，AGPL-3.0-only。

**当前是开发版本，尚未达到完整首版发布标准。** 34 个工具已接入实际处理流程，并在本机生成和检查输出；这不等于所有格式、排版、损坏文件和平台均已验收。详见 [当前验证状态](docs/STATUS.md)。

## 开发

构建机需要 Node.js、Rust、Xcode command line tools（macOS）或 MSVC build tools（Windows）。终端位于项目目录：

```sh
npm ci
npm run check
cargo check --manifest-path src-tauri/Cargo.toml
npm run desktop
```

PDF 引擎组件必须预先放入 `src-tauri/resources/runtime`。应用不会下载缺失组件或调用用户机器上其他安装的转换软件。运行组件不提交到源码仓库；本机开发包使用 macOS arm64 组件。

macOS 组装脚本需要构建环境中的 MuPDF、qpdf、Ghostscript、Tesseract 和独立 headless LibreOffice。用 `BPDF_OFFICE_SOURCE` 指定 LibreOffice `.app` 来源后运行 `npm run prepare:runtime`。当前脚本尚未实现可复现、固定源代码校验和的依赖构建；不能作为正式发行供应链使用。

```sh
cargo build --manifest-path src-tauri/Cargo.toml --bin bpdf-cli
npm run test:engine
npm test
npm run bundle:dev
```

`bundle:dev` 生成本机开发 `.app`，不是已签名公证的发行包。正式入口 `npm run bundle` 先检查发布证据。macOS 14 的兼容性必须检查所有 Mach-O 文件，不能只修改应用声明的系统版本：

```sh
python3 scripts/audit-macos.py src-tauri/target/release/bundle/macos/BPDF.app --report work/app-audit.json --strict
```

## 代码分工

- `src/`：阅读工作区、工具参数、任务与编辑面板。
- `src-tauri/src/documents.rs`：文档快照、撤销重做、另存和渲染缓存。
- `src-tauri/src/engine.rs`：私有请求文件、进程隔离、取消和超时。
- `src-tauri/src/jobs.rs`：34 个工具的处理管线。
- `src-tauri/src/export.rs`：共享版面到 DOCX/PPTX/XLSX/EPUB/Markdown。
- `src-tauri/src/caj.rs`：有界 CAJ/HN 解析。
- `native/reader.c`：MuPDF C 渲染接口及独立进程入口。
- `src-tauri/resources/engine/worker.js`：可信 MuJS 处理脚本；不执行文档 JavaScript。
- `tests/`：实际文件生成、转换及重新打开验证。

## 使用约定

处理默认另存，已有路径会拒绝覆盖。文件密码仅经私有临时文件传给引擎，不放入进程命令行。转换及 OCR 使用单重型任务队列，默认不上传文档；显式开启局域网协作后，仅将共享副本发送给持有邀请码的局域网成员。正文修改删除原文字内容后写入新内容；图片资源按页面复制，图片移动不修改其他页面。

应用的输出仍应由用户核对，尤其是复杂表格、阅读顺序、字体替换和 CAJ。工具报告转换警告；未知格式返回明确错误。打印暂调用系统 PDF 查看器。

## 许可

项目代码见 [LICENSE](LICENSE)。第三方组件仍遵守各自许可。开发运行组件内保留已收集的许可证，正式发行必须补齐完整对应源代码、补丁、构建材料和所有字体/模型的许可清单。见 [第三方组件](docs/THIRD_PARTY.md)。

## 局域网协作（开发功能）

从侧栏“协作”主持当前文档副本，或粘贴邀请码加入。指定本机局域网 IPv4 地址；双方需要运行带完整运行组件的 BPDF。使用 TLS 和邀请证书验证，主持端统一提交修改，冲突时明确提示重新选择内容。跨 macOS/Windows 的网络实现共用 Rust 代码，但 Windows 实机和安装包验收尚未完成。详见状态说明中的限制。

```sh
cargo test --manifest-path src-tauri/Cargo.toml lan_roundtrip -- --ignored
python3 scripts/stress-check.py
```
