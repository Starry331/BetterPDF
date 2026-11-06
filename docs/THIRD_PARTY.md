# 第三方组件清单（开发版本）

| 组件 | 用途 | 官方来源 |
|---|---|---|
| MuPDF | C 渲染、MuJS PDF 对象处理、字体 | https://mupdf.com/ |
| qpdf | 无损结构整理与压缩 | https://github.com/qpdf/qpdf |
| Ghostscript | PDF/A、优化与颜色 | https://ghostscript.com/ |
| Tesseract | OCR | https://github.com/tesseract-ocr/tesseract |
| tessdata_fast | eng、chi_sim、chi_tra、osd 模型 | https://github.com/tesseract-ocr/tessdata_fast |
| LibreOffice | Office 文档输入 | https://www.libreoffice.org/ |
| Tauri / Svelte | 应用框架与 UI | https://tauri.app/ / https://svelte.dev/ |

精确开发二进制版本及文件哈希见 `src-tauri/resources/runtime/manifest.json`，JavaScript 和 Rust 版本由锁文件记录。

保留原始许可证，尊重组件及其传递依赖各自的条件。项目根 LICENSE 只规定项目自身代码，不覆盖第三方代码。当前开发目录的材料还不满足公开二进制发行的完整要求；不得将这份清单视为已完成许可审计。正式发行需逐包附上适用的对应源代码、修改、构建材料、模型/字体许可与声明，并记录可核对的来源及哈希。

新增局域网协作依赖：rustls（https://docs.rs/rustls/0.23.46/rustls/）及 rcgen（https://docs.rs/rcgen/0.14.10/rcgen/），加密提供者为 ring；精确版本和传递依赖见 Cargo.lock。正式分发前也应纳入完整许可/来源清单。

## 固定原生源码构建

MuPDF、qpdf、Ghostscript、Tesseract 与 Leptonica 的源码 URL 和 SHA-256 已记录于 `scripts/native-sources.json`。`scripts/build-native-macos.py --install` 校验源码后编译 macOS 14 arm64 程序，检查仅链接系统动态库，并保留源码中的许可证及声明；源码目录的嵌入依赖也包含在许可证收集范围内。该脚本已在本机完整运行通过。构建工具 CMake 不随应用安装。

这不代替完整对应源码交付：LibreOffice、模型和字体、Rust/JavaScript 传递依赖仍需完成可分发材料清单。不得因某些组件材料已齐全，就将整个项目标记为已完成分发验收。
