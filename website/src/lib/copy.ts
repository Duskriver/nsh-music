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
  note: string;
  screenshot: string;
  how: string;
  howIntro: string;
  steps: [string, string][];
  features: string;
  featuresIntro: string;
  facts: [string, string][];
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
  built: string;
  source: string;
}

export const copy: Record<"en" | "zh-CN", Copy> = {
  en: {
    nav: "Primary",
    language: "Language",
    darkMode: "Toggle dark mode",
    skip: "Skip to content",
    eyebrow: "A native player for Spotify",
    title: "Spotify, natively",
    intro:
      "Spotish is a fast, native desktop client for Spotify — your account, free or Premium, playing through a browser kept out of sight, behind an interface drawn natively by NSH instead of a full web app.",
    getIt: "Get it on GitHub",
    sponsor: "Sponsor",
    note: "Unofficial, and not affiliated with Spotify. A Spotify account is required.",
    screenshot: "Spotish's home screen: a native sidebar, cards, and a player bar.",
    how: "How it works",
    howIntro: "Spotish drives Spotify's own web player and renders its own native UI over it.",
    steps: [
      ["Native shell", "The whole interface — sidebar, lists, the player bar, the shader behind Now Playing — is drawn natively by NSH, not in a web page."],
      ["Your account plays", "A headless browser runs Spotify's web player out of sight; your account does the playing, so the catalogue, your library and the ads all work as Spotify intends."],
      ["One sign-in", "Sign in once (or reuse a browser you're already signed in to); Spotish remembers it and takes over playback."],
    ],
    features: "What you get",
    featuresIntro: "A real client, built to prove NSH for cross-platform apps.",
    facts: [
      ["Native, not a second browser", "The UI is native views on a Rust engine — no Electron, no embedded web app rendering the whole screen."],
      ["Adaptive layout", "A desktop workspace that collapses to an icon rail and becomes a phone layout with a bottom tab bar as the window narrows."],
      ["A living Now Playing", "A full-screen view with a WGSL shader that drifts in colors drawn from the track, and album art that breathes."],
      ["Ads kept, Premium nudged", "Spotish never blocks or skips Spotify's ads. Premium is how you remove them."],
      ["Reuse your login", "Point it at a browser profile you're already signed in to for zero sign-in."],
      ["Follows your theme", "Light and dark follow the system; a Spotify-dark look by default."],
    ],
    compliance: "About this project",
    complianceBody:
      "Spotish is an unofficial, personal experiment, not affiliated with, endorsed by, or sponsored by Spotify. It plays through Spotify's own web player, so a Spotify account is required and Spotify's terms apply. Spotify, its logo and all music, artwork and trademarks belong to Spotify and the rights holders. Spotish keeps Spotify's ads in place.",
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
      omasend: "Paste-and-send file transfer for the local network.",
      noop: "A local-first developer toolkit.",
    },
    footer: "An unofficial Spotify client.",
    built: "Built with",
    source: "Source",
  },
  "zh-CN": {
    nav: "主导航",
    language: "语言",
    darkMode: "切换深色模式",
    skip: "跳到内容",
    eyebrow: "一个原生的 Spotify 播放器",
    title: "原生的 Spotify",
    intro:
      "Spotish 是一个快速、原生的 Spotify 桌面客户端——用你自己的账号(免费或 Premium),让浏览器在后台无感运行,界面则由 NSH 原生绘制,而不是跑一整个网页应用。",
    getIt: "在 GitHub 获取",
    sponsor: "赞助",
    note: "非官方,与 Spotify 无关。需要 Spotify 账号。",
    screenshot: "Spotish 首页:原生侧边栏、卡片与播放条。",
    how: "工作原理",
    howIntro: "Spotish 调度 Spotify 官方网页播放器,并在其之上渲染自己的原生界面。",
    steps: [
      ["原生外壳", "整个界面——侧边栏、列表、播放条、Now Playing 背后的 Shader——都由 NSH 原生绘制,而非网页。"],
      ["用你的账号播放", "一个无头浏览器在后台运行 Spotify 网页播放器;由你的账号播放,因此曲库、资料库和广告都与 Spotify 一致。"],
      ["一次登录", "登录一次(或复用你已登录的浏览器);Spotish 会记住并接管播放。"],
    ],
    features: "你会得到",
    featuresIntro: "一个真实可用的客户端,用来验证 NSH 的跨平台能力。",
    facts: [
      ["原生,而非第二个浏览器", "界面是跑在 Rust 引擎上的原生视图——没有 Electron,不用内嵌网页渲染整个屏幕。"],
      ["自适应布局", "桌面工作区会随窗口变窄收成图标窄条,并切换为带底部标签栏的手机布局。"],
      ["会呼吸的 Now Playing", "全屏视图配上 WGSL Shader,颜色随当前曲目流动,封面轻轻浮动。"],
      ["保留广告,引导 Premium", "Spotish 绝不拦截或跳过 Spotify 的广告。去广告请用 Premium。"],
      ["复用你的登录", "指向一个你已登录的浏览器 profile,即可零登录。"],
      ["跟随系统主题", "明暗跟随系统;默认 Spotify 深色风格。"],
    ],
    compliance: "关于本项目",
    complianceBody:
      "Spotish 是非官方的个人实验项目,与 Spotify 无关联、未获其背书或赞助。它通过 Spotify 官方网页播放器播放,因此需要 Spotify 账号并受其条款约束。Spotify、其标志以及所有音乐、封面和商标归 Spotify 及相应权利人所有。Spotish 保留 Spotify 的广告。",
    premium: "开通 Spotify Premium",
    terms: "Spotify 条款",
    nsh: "用 NSH 构建",
    nshBody:
      "NSH(Native Shell)是一个 Rust UI 框架,让同一个应用通过各平台自己的原生视图渲染——macOS、Windows、Linux、iOS、Android、Web,乃至终端。Spotish 正是用一个真实、非平凡的应用来验证它:一套 Rust 引擎、一套 TSX 界面,处处原生。",
    sponsorHeading: "支持作者",
    sponsorBody:
      "Spotish 免费且开源(Apache-2.0)。如果它对你有用,赞助作者能让这类实验继续下去。",
    sponsorCta: "在 GitHub 赞助",
    related: "更多项目",
    relatedIntro: "同一作者的其他作品。",
    relatedDescriptions: {
      nsh: "绘制 Spotish 的原生 UI 框架。",
      omasend: "面向局域网的「复制即发送」文件传输。",
      noop: "本地优先的开发者工具箱。",
    },
    footer: "一个非官方的 Spotify 客户端。",
    built: "构建自",
    source: "源码",
  },
};
