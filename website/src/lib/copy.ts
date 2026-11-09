// The site copy, English and Simplified Chinese.

export interface Copy {
  nav: string;
  darkMode: string;
  skip: string;
  eyebrow: string;
  title: string;
  intro: string;
  getIt: string;
  homeScreenshot: string;
  playingScreenshot: string;
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
  nsh: string;
  nshBody: string;
  footer: string;
  trademark: string;
  built: string;
  source: string;
}

export const copy: Record<"en" | "zh-CN", Copy> = {
  en: {
    nav: "Primary",
    darkMode: "Toggle dark mode",
    skip: "Skip to content",
    eyebrow: "Built with NSH",
    title: "NetEase Cloud Music, natively",
    intro: "nsh-music is a native NetEase Cloud Music client. Search for songs, open your library and play music in an NSH interface backed by a Rust audio engine.",
    getIt: "Get it on GitHub",
    homeScreenshot: "The nsh-music desktop app with its library sidebar, home recommendations and player bar.",
    playingScreenshot: "Now Playing with the album cover and time-synced lyrics.",
    install: "Install",
    installTabs: "Install method",
    installUnix: "macOS / Linux",
    installWindows: "Windows",
    installNote: "The installer picks your platform and checks the release's checksum. No sudo required.",
    installRequirements: "Apple silicon on macOS 14 or later, x86_64 Linux with GTK 4.14 or later and ALSA, or x86_64 Windows. macOS builds may require approval in Privacy & Security on first launch.",
    readScript: "Read the installer",
    copy: "Copy",
    copied: "Copied",
    copyFailed: "Select to copy",
    how: "How it works",
    howIntro: "The Rust engine connects to NetEase Cloud Music and decodes audio with rodio.",
    steps: [
      ["Scan to sign in", "Use the NetEase Cloud Music mobile app to scan the desktop QR code."],
      ["Find your music", "Search for songs, albums and artists, or open playlists from your library."],
      ["Play locally", "The Rust player decodes audio on your device."],
    ],
    features: "What you get",
    featuresIntro: "Browse your library and follow lyrics as the music plays.",
    facts: [
      ["Synced lyrics", "Follow time-coded lyrics in Now Playing."],
      ["Album artwork", "The current cover colors the player visuals."],
      ["System theme", "Light and dark appearance follow the system."],
    ],
    platformsLabel: "Platforms",
    platforms: [
      ["Desktop", "Native playback on macOS, Linux and Windows."],
      ["Terminal", "A keyboard-driven interface using the same native player."],
      ["Web and mobile", "An offline demo with sample music data."],
    ],
    compliance: "About this project",
    complianceBody: "nsh-music is an unofficial client for NetEase Cloud Music. Music, lyrics, artwork and trademarks belong to their respective rights holders.",
    nsh: "Built with NSH",
    nshBody: "NSH (Native Shell) renders the interface using each platform’s native views. The app combines a Rust engine and a TSX interface.",
    footer: "A native NetEase Cloud Music client.",
    trademark: "NetEase Cloud Music and its trademarks belong to their respective rights holders.",
    built: "Built with",
    source: "Source",
  },
  "zh-CN": {
    nav: "主导航",
    darkMode: "切换深色模式",
    skip: "跳到内容",
    eyebrow: "基于 NSH 构建",
    title: "原生网易云音乐客户端",
    intro: "nsh-music 用 NSH 渲染原生界面，由 Rust 音频引擎播放网易云音乐。你可以搜索歌曲、浏览自己的歌单，或在播放时查看同步歌词。",
    getIt: "在 GitHub 获取",
    homeScreenshot: "nsh-music 桌面端首页，显示曲库侧栏、推荐歌单和播放栏。",
    playingScreenshot: "正在播放页显示专辑封面和逐行同步的歌词。",
    install: "安装",
    installTabs: "安装方式",
    installUnix: "macOS / Linux",
    installWindows: "Windows",
    installNote: "安装脚本会选择对应平台，并校验发行包的校验和。无需 sudo。",
    installRequirements: "支持 macOS 14 及以上的 Apple 芯片设备、具备 GTK 4.14 及以上版本和 ALSA 的 x86_64 Linux，以及 x86_64 Windows。macOS 首次启动可能需要在「隐私与安全性」中允许。",
    readScript: "查看安装脚本",
    copy: "复制",
    copied: "已复制",
    copyFailed: "请手动复制",
    how: "工作原理",
    howIntro: "Rust 引擎连接网易云音乐，并通过 rodio 解码播放。",
    steps: [
      ["扫码登录", "使用网易云音乐手机 App 扫描桌面端二维码。"],
      ["找到想听的音乐", "搜索歌曲、专辑和歌手，或打开自己的歌单。"],
      ["本机播放", "Rust 播放器在设备上解码音频。"],
    ],
    features: "你会得到",
    featuresIntro: "从曲库到播放页，随时查看正在播放的歌曲与歌词。",
    facts: [
      ["同步歌词", "在正在播放页查看逐行同步的歌词。"],
      ["封面配色", "播放动效随当前歌曲封面的颜色变化。"],
      ["系统主题", "浅色和深色外观随系统设置切换。"],
    ],
    platformsLabel: "平台",
    platforms: [
      ["桌面端", "在 macOS、Linux 和 Windows 上使用原生播放器。"],
      ["终端", "通过键盘操作同一个原生播放引擎。"],
      ["Web 与移动端", "内置示例曲库的离线演示。"],
    ],
    compliance: "关于本项目",
    complianceBody: "nsh-music 是网易云音乐的非官方客户端。音乐、歌词、封面和商标归各自权利人所有。",
    nsh: "用 NSH 构建",
    nshBody: "NSH（Native Shell）使用各平台的原生视图渲染界面。应用由 Rust 引擎和 TSX 界面组成。",
    footer: "原生网易云音乐客户端。",
    trademark: "网易云音乐及其商标归相应权利人所有。",
    built: "构建自",
    source: "源码",
  },
};
