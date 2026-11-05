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
