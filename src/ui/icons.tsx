// The app's icons: one family of 24×24 line glyphs (Lucide geometry), rendered
// as an SVG data-URI through <Image>, with the color baked in so they follow
// the theme. TSX has no inline-SVG element, so the color is substituted rather
// than `currentColor`.

import { colorScheme, Image, token } from "nsh";

export type IconName =
  | "home" | "search" | "library" | "play" | "pause" | "skipBack" | "skipForward"
  | "shuffle" | "repeat" | "repeatOne" | "heart" | "heartFilled" | "volume" | "volumeLow"
  | "volumeMute" | "chevronLeft" | "chevronRight" | "chevronDown" | "music" | "user"
  | "logIn" | "logOut" | "disc" | "clock" | "alert" | "close";

const BODY: Record<IconName, string> = {
  home: `<path d="M3 10.5 12 3l9 7.5"/><path d="M5 9.5V20a1 1 0 0 0 1 1h4v-6h4v6h4a1 1 0 0 0 1-1V9.5"/>`,
  search: `<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>`,
  library: `<path d="M4 4v16"/><path d="M9 4v16"/><path d="m14 4.5 5.5 15"/>`,
  play: `<path d="M7 4.5v15a.8.8 0 0 0 1.2.7l12-7.5a.8.8 0 0 0 0-1.4l-12-7.5A.8.8 0 0 0 7 4.5Z" fill="@" stroke="none"/>`,
  pause: `<rect x="6" y="4" width="4" height="16" rx="1" fill="@" stroke="none"/><rect x="14" y="4" width="4" height="16" rx="1" fill="@" stroke="none"/>`,
  skipBack: `<path d="M19 20 9 12l10-8v16Z" fill="@" stroke="none"/><path d="M5 19V5"/>`,
  skipForward: `<path d="m5 4 10 8-10 8V4Z" fill="@" stroke="none"/><path d="M19 5v14"/>`,
  shuffle: `<path d="M2 18h1.4a4 4 0 0 0 3.3-1.8l6.6-9.4A4 4 0 0 1 16.6 5H22"/><path d="m18 2 4 3-4 3"/><path d="M2 6h1.4a4 4 0 0 1 3.3 1.8l.8 1.2"/><path d="M22 19h-5.4a4 4 0 0 1-3.3-1.8l-.8-1.2"/><path d="m18 16 4 3-4 3"/>`,
  repeat: `<path d="m17 2 4 4-4 4"/><path d="M3 11v-1a4 4 0 0 1 4-4h14"/><path d="m7 22-4-4 4-4"/><path d="M21 13v1a4 4 0 0 1-4 4H3"/>`,
  repeatOne: `<path d="m17 2 4 4-4 4"/><path d="M3 11v-1a4 4 0 0 1 4-4h14"/><path d="m7 22-4-4 4-4"/><path d="M21 13v1a4 4 0 0 1-4 4H3"/><path d="M11 10h1v4"/>`,
  heart: `<path d="M19 14c1.5-1.5 3-3.2 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.8 0-3 .5-4.5 2-1.5-1.5-2.7-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4 3 5.5l7 7Z"/>`,
  heartFilled: `<path d="M19 14c1.5-1.5 3-3.2 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.8 0-3 .5-4.5 2-1.5-1.5-2.7-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4 3 5.5l7 7Z" fill="@" stroke="none"/>`,
  volume: `<path d="M11 4.7a.7.7 0 0 0-1.2-.5L6.4 7.6A2 2 0 0 1 5 8.2H3a1 1 0 0 0-1 1v5.6a1 1 0 0 0 1 1h2a2 2 0 0 1 1.4.6l3.4 3.4a.7.7 0 0 0 1.2-.5Z"/><path d="M16 9a5 5 0 0 1 0 6"/><path d="M19.4 18.4a9 9 0 0 0 0-12.8"/>`,
  volumeLow: `<path d="M11 4.7a.7.7 0 0 0-1.2-.5L6.4 7.6A2 2 0 0 1 5 8.2H3a1 1 0 0 0-1 1v5.6a1 1 0 0 0 1 1h2a2 2 0 0 1 1.4.6l3.4 3.4a.7.7 0 0 0 1.2-.5Z"/><path d="M16 9a5 5 0 0 1 0 6"/>`,
  volumeMute: `<path d="M11 4.7a.7.7 0 0 0-1.2-.5L6.4 7.6A2 2 0 0 1 5 8.2H3a1 1 0 0 0-1 1v5.6a1 1 0 0 0 1 1h2a2 2 0 0 1 1.4.6l3.4 3.4a.7.7 0 0 0 1.2-.5Z"/><path d="m22 9-6 6"/><path d="m16 9 6 6"/>`,
  chevronLeft: `<path d="m15 18-6-6 6-6"/>`,
  chevronRight: `<path d="m9 18 6-6-6-6"/>`,
  chevronDown: `<path d="m6 9 6 6 6-6"/>`,
  music: `<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>`,
  user: `<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>`,
  logIn: `<path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4"/><path d="m10 17 5-5-5-5"/><path d="M15 12H3"/>`,
  logOut: `<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><path d="m16 17 5-5-5-5"/><path d="M21 12H9"/>`,
  disc: `<circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="2"/>`,
  clock: `<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>`,
  alert: `<circle cx="12" cy="12" r="10"/><path d="M12 8v4"/><path d="M12 16h.01"/>`,
  close: `<path d="M18 6 6 18"/><path d="m6 6 12 12"/>`,
};

function dataUri(name: IconName, color: string): string {
  const body = BODY[name].replaceAll("@", color);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="${color}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

export interface IconProps {
  name: IconName;
  /** Points (default 24). */
  size?: number;
  /** A theme token name (default `"foreground"`) or a literal color. */
  color?: string;
}

/** The named glyph, in the theme color. */
export function Icon(props: IconProps) {
  const color = () => {
    const name = props.color ?? "foreground";
    // Re-read colorScheme so the baked color follows light/dark.
    colorScheme();
    return (token(name as never) as unknown as string) ?? name;
  };
  const size = () => props.size ?? 24;
  return <Image src={dataUri(props.name, color())} style={{ width: size(), height: size() }} />;
}
