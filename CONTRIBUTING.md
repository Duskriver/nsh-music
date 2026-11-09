# 开发与贡献指南 (CONTRIBUTING.md)

欢迎参与 `nsh-music` 的开发与改进！在提交代码前，请先阅读以下项目结构、开发环境配置与验证规范。

## 环境准备

1. **Rust 工具链**：通过 [rustup](https://rustup.rs) 安装（Linux 需安装 GTK 4.14+ 及 ALSA 开发包，详见 [NSH 安装指南](https://nsh.dev/docs/installation.md)）。
2. **`nsh` CLI (nightly)**：本项目跟随 NSH `main` 分支，请安装 nightly 版本的 CLI：

   ```sh
   curl -fsSL https://nsh.dev/install.sh | sh -s -- --nightly
   nsh --version
   ```

### NSH 版本同步说明

`Cargo.toml` 从 Git 引用 `nsh-core`、`nsh-engine` 和 `nsh-testing`。`Cargo.lock` 中锁定的提交哈希需要与 `nsh --version` 打印的 CLI 构建哈希保持一致。升级 `nsh` CLI 后，请运行：

```sh
cargo update -p nsh-core --precise <hash>
```

---

## 运行方式

```sh
nsh run                       # 构建 Rust 引擎并打包 TSX 界面，在桌面端运行
NSH_MUSIC_DEMO=1 nsh run        # 演示模式：使用离线录制的示例数据运行（无需网络和账号）
nsh run --target tui          # 在终端中运行 TUI 字符界面
```

### 数据存储路径

应用会话（`netease-session.json`）存储在各系统的标准应用数据目录，可通过环境变量 `NSH_MUSIC_DATA_DIR` 覆盖。
- **macOS**：`~/Library/Application Support/io.github.duskriver.nsh-music`
- **Linux**：`$XDG_DATA_HOME/nsh-music`（默认 `~/.local/share/nsh-music`）
- **Windows**：`%LOCALAPPDATA%\nsh-music`

首次使用新默认目录时，应用会迁移旧版保存的网易云登录会话。

---

## 代码检查与测试

在提交代码或发起 PR 前，请确保以下检查全部通过：

```sh
nsh check                              # 执行 cargo check --all-targets 与 tsc 类型检查
nsh build                              # 检查 Rust 导出文档注释并打包 TSX
nsh test                               # 运行 cargo test 及所有 *.test.tsx 无头测试
cargo test                             # 仅运行 Rust 单元测试
```

测试永远不需要真实的网易云账号或联网环境：Rust 单元测试验证加密与协议解析，UI 端到端测试（`tests/app.test.tsx`）运行在离线 `DemoBackend` 上。运行检查时请将 `TMPDIR` 指向仓库内的 `target/agents/tmp`。

---

## 项目目录结构

### Rust 层（原生引擎与平台集成）

- `src/music/netease/` — **纯原生网易云音乐引擎**：
  - `crypto.rs`：EAPI 请求的 `AES-128-ECB`（PKCS7）加密与 `MD5` 摘要实现。
  - `client.rs`：网易云 Web API 与 EAPI 客户端，包含二维码扫码登录、Cookie 持久化、首页推荐、搜索、歌单/专辑/歌手详情、红心曲库、LRC 歌词解析与音频直链获取。
  - `player.rs`：基于 `rodio` 的原生音频播放器与播放队列管理器，通过 `MeteredSource` 实时分流 PCM 音频采样。
  - `session.rs`：后台会话线程，对接 UI 指令并发布 `Snapshot` 状态更新。
- `src/music/model.rs` — 跨语言数据模型（通过 `#[derive(JsObject)]` 导出给 TSX）。
- `src/music/demo.rs` — 离线演示与测试数据源。
- `src/audio.rs` — 实时音频频谱分析器（`AudioMeter`），直接消费播放器分流的音频采样，计算 16 段频谱与节拍脉冲。
- `src/media.rs` — 系统媒体控制与多媒体快捷键集成（MPRIS / macOS MediaPlayer / Windows SMTC）。
- `src/theme.rs` — 系统主题监听与色彩映射。
- `src/exports.rs` — 向 TSX 导出的原生模块入口（`nsh-music/native`）。

### TSX 层（原生用户界面）

- `src/backend/` — `Backend` 契约接口（`types.ts`）及其实现：`EngineBackend`（桌面与终端）和 `DemoBackend`（Web、移动端与自动化测试）。
- `src/state/` — 响应式状态存储（`engine.ts`、`player.ts`、`session.ts`、`feedback.ts`、`visual.ts`）。
- `src/data/` — 账号隔离的缓存与响应式数据查询（`queries.ts`、`cache.ts`）。
- `src/components/` — 基于主题 Token 封装的通用控件、图标、虚拟化曲目列表与播放器组件。
- `src/pages/` — 首页（`home.tsx`）、搜索（`search.tsx`）、歌单/专辑/歌手详情（`collection.tsx`）及扫码登录/关于弹窗（`dialogs.tsx`）。
- `src/shells/` — 桌面端（`desktop.tsx`）、手机端（`phone.tsx`）与终端（`terminal.tsx`）自适应外壳。
- `theme.json` — 浅色与深色模式的语义化颜色与尺寸 Token。
- `tests/` — 端到端 UI 测试（`app.test.tsx`）、着色器编译测试（`shaders.rs`）与性能基准测试（`bench.test.tsx`）。

---

## 提交规范

- 保持每个提交聚焦于单一改动，提交信息格式为 `<area>: <简要描述改动内容>`（例如 `engine: Add fallback for desktop LRC lyrics`、`player: Fix repeat track seek position`）。
- 任何导出到 TSX 的 Rust 类型、字段和方法都必须包含 `///` 文档注释，确保 `nsh build` 零警告。
