// The About panel: the Spotify disclaimer first, then a short word on NSH,
// with links.

import { Scroll, Text, View } from "nsh";
import { Button } from "nsh/base";
import { store } from "../ui/store";
import { PillButton } from "../ui/controls";
import { Icon } from "../ui/icons";

const REPO = "https://github.com/huacnlee/nshtify";
const NSH = "https://nsh.dev";
const TERMS = "https://www.spotify.com/legal/end-user-agreement/";
const SPOTIFY = "https://www.spotify.com/";

function Link(props: { label: string; url: string }) {
  return (
    <Text role="link" onPress={() => store.openExternal(props.url)} class="text-sm font-semibold text-primary hover:underline">
      {props.label}
    </Text>
  );
}

function LinkRow(props: { label: string; url: string }) {
  return (
    <Button
      onPress={() => store.openExternal(props.url)}
      class="h-11 flex-row items-center justify-between rounded-(--radius) bg-secondary px-4 hover:bg-row-hover active:opacity-70 focus:outline focus:outline-2 focus:outline-ring"
    >
      <Text class="text-sm font-medium text-foreground">{props.label}</Text>
      <View class="text-muted-foreground"><Icon name="chevronRight" size={16} color="muted-foreground" /></View>
    </Button>
  );
}

export function About() {
  return (
    <If when={store.about()}>
      <View class="absolute inset-0 items-center justify-center bg-shell/85 p-4" onPress={() => store.about.set(false)}>
        <View class="max-h-[640px] w-[440px] max-w-[440px] overflow-hidden rounded-(--radius-panel) bg-popover shadow-2xl" onPress={() => {}}>
          <Scroll class="flex-1">
            <View class="gap-5 p-6">
              {/* Header */}
              <View class="flex-row items-center justify-between">
                <View class="flex-row items-center gap-3">
                  <View class="text-primary"><Icon name="music" size={26} color="primary" /></View>
                  <View>
                    <Text class="text-xl font-extrabold text-foreground">Spotish</Text>
                    <Text class="text-xs text-muted-foreground">A native player for Spotify</Text>
                  </View>
                </View>
                <Button accessibilityLabel="Close" onPress={() => store.about.set(false)} class="size-8 items-center justify-center rounded-full text-muted-foreground hover:bg-row-hover active:opacity-60">
                  <Icon name="close" size={18} color="muted-foreground" />
                </Button>
              </View>

              {/* Spotify disclaimer, first */}
              <View class="gap-2">
                <Text class="text-sm font-bold text-foreground">Not affiliated with Spotify</Text>
                <Text class="text-sm leading-[21px] text-muted-foreground">
                  Spotish is an unofficial, personal project, not affiliated with, endorsed by, or sponsored by Spotify.
                  It plays through Spotify's own web player, so a Spotify account is needed and Spotify's terms apply.
                  Spotify, its logo and all music, artwork and trademarks belong to Spotify and the rights holders.
                </Text>
                <View class="flex-row flex-wrap gap-x-4 gap-y-1">
                  <Link label="Spotify Terms" url={TERMS} />
                  <Link label="spotify.com" url={SPOTIFY} />
                </View>
              </View>

              {/* Premium */}
              <View class="gap-2 rounded-(--radius-card) bg-secondary p-4">
                <Text class="text-sm font-semibold text-foreground">Keeping the ads? Spotify Premium removes them.</Text>
                <Text class="text-sm leading-[21px] text-muted-foreground">
                  Spotish never blocks Spotify's ads. Premium removes them, plays any track on demand, and supports artists.
                </Text>
                <View class="flex-row pt-1">
                  <PillButton label="Get Spotify Premium" primary onPress={() => store.openPremium()} />
                </View>
              </View>

              {/* NSH, short */}
              <View class="flex-row flex-wrap items-baseline gap-x-1">
                <Text class="text-sm text-muted-foreground">Built with</Text>
                <Link label="NSH" url={NSH} />
                <Text class="text-sm leading-[21px] text-muted-foreground">
                  — a Rust framework for native UIs on desktop, mobile, web and the terminal. This window is all native.
                </Text>
              </View>

              <View class="gap-2">
                <LinkRow label="Project on GitHub" url={REPO} />
                <LinkRow label="Learn more about NSH" url={NSH} />
              </View>
            </View>
          </Scroll>
        </View>
      </View>
    </If>
  );
}
