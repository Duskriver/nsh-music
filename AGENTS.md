# AGENTS.md

`nsh-music` 是基于 NSH (Native Shell) 混合模式（Rust + TSX）构建的纯原生网易云音乐客户端：底层由 Rust 驱动网易云音乐 API 协议栈（`src/music/netease/`）与 `rodio` 原生音频解码器，上层为 TSX 原生界面。请先阅读 `CONTRIBUTING.md` 了解项目结构与检查命令，阅读 `docs/DESIGN.md` 了解视觉与交互设计规范。

开发与维护必须遵守以下核心规则：

- **提交前必须运行完整检查**：执行 `nsh check`、`nsh build` 和 `nsh test`，确保所有编译、类型检查、Rust 单元测试和 TSX 端到端测试零错误、零警告通过。
- **严禁写入 `/tmp` 目录**：所有临时文件、日志和构建缓存必须放在 `target/agents/` 下，运行测试时请将 `TMPDIR` 指向 `target/agents/tmp`。
- **保持导出类型文档完整**：`src/exports.rs` 和 `src/music/model.rs` 中所有导出到 TSX 的结构体、枚举、字段和方法都必须带有 `///` 文档注释（注意不要在 `#[class] impl` 块本身上加 `///`）。
- **测试不依赖真实网络或账号**：UI 自动化测试（`tests/app.test.tsx`）运行在 `DemoBackend` 上，修改界面文案或交互时请确保不破坏现有无头测试断言。
