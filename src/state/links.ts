// Links that leave the app: opened in the system browser, https only.

import { openUrl } from "nsh";

export const NSH_URL = "https://nsh.dev";
export const REPO_URL = "https://github.com/Duskriver/nsh-music";

/** Opens `url` in the system browser; anything but https is refused. */
export function openExternal(url: string): void {
  if (!url.startsWith("https://")) {
    console.warn(`nsh-music: refused to open a non-https link: ${url}`);
    return;
  }
  openUrl(url).catch((error) => console.warn("nsh-music: couldn't open a link:", String(error)));
}
