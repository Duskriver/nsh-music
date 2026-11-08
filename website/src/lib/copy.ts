// The site copy, English and Simplified Chinese.

export interface Copy {
  nav: string;
  language: string;
  darkMode: string;
  skip: string;
  eyebrow: string;
  title: string;
  intro: string;
  getIt: string;
  sponsor: string;
  premiumNote: string;
  screenshot: string;
  screensLabel: string;
  install: string;
  installTabs: string;
  installUnix: string;
  installWindows: string;
  installNote: string;
  installRequirements: string;
  readScript: string;
  copy: string;
  copied: string;
  copyFailed: string;
  artCredit: string;
  noticeLabel: string;
  screens: { artist: string; liked: string; nowPlaying: string; narrow: string; nowPlayingNarrow: string };
  how: string;
  howIntro: string;
  steps: [string, string][];
  features: string;
  featuresIntro: string;
  facts: [string, string][];
  platformsLabel: string;
  platforms: [string, string][];
  compliance: string;
  complianceBody: string;
  premium: string;
  terms: string;
  nsh: string;
  nshBody: string;
  sponsorHeading: string;
  sponsorBody: string;
  sponsorCta: string;
  related: string;
  relatedIntro: string;
  relatedDescriptions: Record<string, string>;
  footer: string;
  trademark: string;
  built: string;
  source: string;
}

export const copy: Record<"en" | "zh-CN", Copy> = {
  en: {
    nav: "Primary",
    language: "Language",
    darkMode: "Toggle dark mode",
    skip: "Skip to content",
    eyebrow: "Built with NSH",
    title: "A native Spotify",
    intro:
      "Spotish is an unofficial, native desktop client that works with your Spotify account, free or Premium. Spotify's own web player plays out of sight in your installed browser, behind an interface drawn natively by NSH instead of a full web app.",
    getIt: "Get it on GitHub",
    sponsor: "Sponsor",
    premiumNote: "Spotish keeps Spotify's ads. To remove them, play any track on demand and support artists, get Premium.",
    screenshot: "Spotish's home screen: a native sidebar, cards, and a player bar.",
    screensLabel: "More screens",
    install: "Install",
    installTabs: "Install method",
    installUnix: "macOS / Linux",
    installWindows: "Windows",
    installNote: "The installer picks your platform and checks the release's checksum. No sudo required.",
    installRequirements: "Apple silicon on macOS 14 or later, x86_64 Linux with GTK 4.14 or later, or x86_64 Windows; plus Chrome, Chromium, Edge or Brave, and a Spotify account. macOS builds are not notarized, so the first launch may need approval in Privacy & Security.",
    readScript: "Read the installer",
    copy: "Copy",
    copied: "Copied",
    copyFailed: "Select to copy",
    noticeLabel: "How Spotish plays",
    artCredit: "Spotish wraps Spotify's official web player: it plays with your own account, under its terms, ads kept in full.\nAn unofficial project, not affiliated with Spotify; Spotify is a trademark of Spotify AB.",
    screens: {
      artist: "An artist: their top tracks, the one playing marked.",
      liked: "Liked Songs, with the album column in a wide window.",
      nowPlaying: "Now Playing: the cover large, the music's light rising behind the controls.",
      narrow: "A narrow window: the sidebar folded away, your picture and menu in the corner.",
      nowPlayingNarrow: "Now Playing in a narrow window.",
    },
    how: "How it works",
    howIntro: "Spotish drives Spotify's own web player and shows its own native interface for it.",
    steps: [
      ["Native shell", "The whole interface — sidebar, lists, the player bar and Now Playing — is drawn natively by NSH, not in a web page."],
      ["Your account plays", "Your installed Chrome, Chromium, Edge or Brave runs Spotify's web player out of sight; your account does the playing, so the catalogue, your library and the ads all work as Spotify intends."],
      ["One sign-in", "Sign in once (or reuse a browser you're already signed in to); Spotish remembers it and takes over playback."],
    ],
    features: "What you get",
    featuresIntro: "A real client, built to prove NSH for cross-platform apps.",
    facts: [
      ["Native, not a second browser", "The UI is native views on a Rust engine — no Electron, no embedded web app rendering the whole screen."],
      ["Adaptive layout", "A desktop workspace that collapses to an icon rail and becomes a phone layout with a bottom tab bar as the window narrows."],
      ["A focused Now Playing", "A full-screen view built around the album art, with the track's own colors and motion that stays out of the way."],
      ["Ads kept, Premium nudged", "Spotish never blocks or skips Spotify's ads. Premium is how you remove them."],
      ["Reuse your login", "Point it at a browser profile you're already signed in to for zero sign-in."],
      ["Follows your theme", "Light and dark follow the system."],
    ],
    platformsLabel: "Platforms",
    platforms: [
      ["Desktop", "Linux and macOS play music; Windows plays through Edge."],
      ["Terminal", "The same app as a terminal UI, playing through the desktop engine."],
      ["Web and mobile", "A demo with sample data, and a planned remote control for your desktop Spotish: Spotify's free tier plays only in its web player, with DRM, in a real browser."],
    ],
    compliance: "About this project",
    complianceBody:
      "Spotish is an unofficial, personal experiment, not affiliated with, endorsed by, or sponsored by Spotify. It plays through Spotify's own web player, so a Spotify account is required and Spotify's terms apply. Spotish never blocks or skips Spotify's ads — Spotify Premium is how you remove them, and how you support the artists you listen to. All music and artwork belong to their rights holders.",
    premium: "Get Spotify Premium",
    terms: "Spotify Terms",
    nsh: "Built with NSH",
    nshBody:
      "NSH (Native Shell) is a Rust UI framework that renders one app through each platform's own native views — on macOS, Windows, Linux, iOS, Android, the web and even the terminal. Spotish exists to prove it out on a real, non-trivial app: one Rust engine, one TSX UI, native everywhere.",
    sponsorHeading: "Support the work",
    sponsorBody:
      "Spotish is free and open source (Apache-2.0). If it's useful to you, sponsoring the author keeps experiments like this going.",
    sponsorCta: "Sponsor on GitHub",
    related: "More projects",
    relatedIntro: "Other things built by the same author.",
    relatedDescriptions: {
      nsh: "The native UI framework that draws Spotish.",
      omasend: "A native LocalSend client. Share files, folders, screenshots and text over your local network, with no account or cloud upload.",
      omamail: "A native email and calendar app with multiple accounts and keyboard navigation. Runs as an Omarchy plugin or a standalone desktop app.",
      noop: "Your own AI team. Claude Code and Codex agents on your own computers do the daily work and only ask when something really needs you.",
      disktree: "A native disk-space explorer for macOS and Linux. See what fills your disk as a nested treemap, and review every path before removing anything.",
      cargo: "A drop-in Cargo fork: every git worktree of a project shares one build directory, which stays under a size limit on its own.",
    },
    footer: "An unofficial client for Spotify.",
    trademark: "Unofficial — not affiliated with, endorsed by or sponsored by Spotify. Spotify is a trademark of Spotify AB.",
    built: "Built with",
    source: "Source",
  },
  "zh-CN": {
    nav: "主导航",
    language: "语言",
    darkMode: "切换深色模式",
    skip: "跳到内容",
    eyebrow: "基于 NSH 构建",
    title: "原生的 Spotify",
    intro:
      "Spotish 是一个非官方的原生桌面客户端，配合你的 Spotify 账号（免费或 Premium）使用。Spotify 自己的网页播放器在你已安装的浏览器中于后台运行，界面则由 NSH 原生绘制，而不是跑一整个网页应用。",
    getIt: "在 GitHub 获取",
    sponsor: "赞助",
    premiumNote: "Spotish 保留 Spotify 的广告。想去掉广告、任意点播并支持艺人，请开通 Premium。",
    screenshot: "Spotish 首页：原生侧边栏、卡片与播放条。",
    screensLabel: "更多截图",
    install: "安装",
    installTabs: "安装方式",
    installUnix: "macOS / Linux",
    installWindows: "Windows",
    installNote: "安装脚本会选择对应平台，并校验发行包的校验和。无需 sudo。",
    installRequirements: "需要 macOS 14 及以上的 Apple 芯片设备、带 GTK 4.14 及以上的 x86_64 Linux，或 x86_64 Windows；另需 Chrome、Chromium、Edge 或 Brave，以及 Spotify 账号。macOS 版本未经公证，首次启动可能需要在“隐私与安全性”中允许。",
    readScript: "查看安装脚本",
    copy: "复制",
    copied: "已复制",
    copyFailed: "请手动复制",
    noticeLabel: "Spotish 如何播放",
    artCredit: "Spotish 基于 Spotify 官方网页播放器包装实现，用你自己的账号播放、遵守其条款，完整保留广告。本项目为非官方，与 Spotify 无关联；Spotify 是 Spotify AB 的商标。",
    screens: {
      artist: "歌手页：热门歌曲，正在播放的一首有标记。",
      liked: "已喜欢的歌曲，宽窗口下显示专辑列。",
      nowPlaying: "正在播放：大封面，音乐的光在控件后升起。",
      narrow: "窄窗口：侧栏整个收起，头像和菜单移到左上角。",
      nowPlayingNarrow: "窄窗口下的正在播放。",
    },
    how: "工作原理",
    howIntro: "Spotish 调度 Spotify 官方网页播放器，并为它提供自己的原生界面。",
    steps: [
      ["原生外壳", "整个界面——侧边栏、列表、播放条与 Now Playing——都由 NSH 原生绘制，而非网页。"],
      ["用你的账号播放", "你已安装的 Chrome、Chromium、Edge 或 Brave 在后台运行 Spotify 网页播放器；由你的账号播放，因此曲库、资料库和广告都与 Spotify 一致。"],
      ["一次登录", "登录一次（或复用你已登录的浏览器）；Spotish 会记住并接管播放。"],
    ],
    features: "你会得到",
    featuresIntro: "一个真实可用的客户端，用来验证 NSH 的跨平台能力。",
    facts: [
      ["原生，而非第二个浏览器", "界面是跑在 Rust 引擎上的原生视图——没有 Electron，不用内嵌网页渲染整个屏幕。"],
      ["自适应布局", "桌面工作区会随窗口变窄收成图标窄条，并切换为带底部标签栏的手机布局。"],
      ["专注的 Now Playing", "以封面为中心的全屏视图，取用曲目本身的颜色，动效克制不打扰。"],
      ["保留广告，引导 Premium", "Spotish 绝不拦截或跳过 Spotify 的广告。去广告请用 Premium。"],
      ["复用你的登录", "指向一个你已登录的浏览器 profile，即可零登录。"],
      ["跟随系统主题", "明暗跟随系统。"],
    ],
    platformsLabel: "平台",
    platforms: [
      ["桌面", "Linux 与 macOS 可以播放；Windows 通过 Edge 播放。"],
      ["终端", "同一个应用的终端界面，由桌面引擎播放。"],
      ["Web 与移动端", "使用示例数据的演示版，并计划作为桌面 Spotish 的遥控器：Spotify 免费版只能在真实浏览器中的官方网页播放器（带 DRM）里播放。"],
    ],
    compliance: "关于本项目",
    complianceBody:
      "Spotish 是非官方的个人实验项目，与 Spotify 无关联、未获其背书或赞助。它通过 Spotify 官方网页播放器播放，因此需要 Spotify 账号并受其条款约束。Spotish 绝不拦截或跳过 Spotify 的广告——开通 Spotify Premium 才是去广告、支持艺人的方式。所有音乐与封面归相应权利人所有。",
    premium: "开通 Spotify Premium",
    terms: "Spotify 条款",
    nsh: "用 NSH 构建",
    nshBody:
      "NSH（Native Shell）是一个 Rust UI 框架，让同一个应用通过各平台自己的原生视图渲染——macOS、Windows、Linux、iOS、Android、Web，乃至终端。Spotish 正是用一个真实、非平凡的应用来验证它：一套 Rust 引擎、一套 TSX 界面，处处原生。",
    sponsorHeading: "支持作者",
    sponsorBody:
      "Spotish 免费且开源（Apache-2.0）。如果它对你有用，赞助作者能让这类实验继续下去。",
    sponsorCta: "在 GitHub 赞助",
    related: "更多项目",
    relatedIntro: "同一作者的其他作品。",
    relatedDescriptions: {
      nsh: "绘制 Spotish 的原生 UI 框架。",
      omasend: "原生 LocalSend 客户端。通过局域网分享文件、文件夹、截图和文字，无需账号，也不上传云端。",
      omamail: "原生邮件与日历应用，支持多账号和键盘操作。可作为 Omarchy 插件或独立桌面应用运行。",
      noop: "你自己的 AI 团队。Claude Code 和 Codex 智能体在你自己的电脑上处理日常工作，只在真正需要你时才来询问。",
      disktree: "macOS 与 Linux 上的原生磁盘空间浏览器。用嵌套 treemap 看清磁盘被什么占满，删除前逐条核对路径。",
      cargo: "可直接替换的 Cargo 分支：同一项目的所有 git worktree 共享一个构建目录，并自动控制在大小上限内。",
    },
    footer: "一个非官方的 Spotify 客户端。",
    trademark: "非官方——与 Spotify 无关联，未获其背书或赞助。Spotify 是 Spotify AB 的商标。",
    built: "构建自",
    source: "源码",
  },
};
