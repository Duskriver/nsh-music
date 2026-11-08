// What several screens share: item cards, track rows, and a page message.

import { List, Scroll, Text, View } from "nsh";
import { Button } from "nsh/base";
import { store } from "../ui/store";
import { Cover, SectionTitle } from "../ui/controls";
import { Icon, type IconName } from "../ui/icons";
import { clock, type Item, opens, type Track } from "../ui/types";

function fallbackIcon(kind: string): IconName {
  if (kind === "artist") return "user";
  if (kind === "album") return "disc";
  return "music";
}

/** Opens an item's page, or plays it when it has none (a track). */
export function activate(item: Item) {
  if (opens(item)) store.open({ name: "collection", uri: item.uri });
  else store.playItem(item);
}

/** One card: picture, name, and what it is. */
export function Card(props: { item: Item }) {
  const item = () => props.item;
  return (
    <Button
      accessibilityLabel={`${item().name} ${item().subtitle}`}
      onPress={() => activate(item())}
      class="w-[180px] shrink-0 gap-2 rounded-(--radius-card) p-3 hover:bg-card active:opacity-80 focus:outline focus:outline-2 focus:outline-ring"
    >
      <Cover src={() => item().image} round={item().kind === "artist"} fallback={fallbackIcon(item().kind)} />
      <View class="gap-1">
        <Text class="line-clamp-1 text-sm font-semibold text-foreground">{item().name}</Text>
        <Text class="line-clamp-2 text-sm text-muted-foreground">{item().subtitle}</Text>
      </View>
    </Button>
  );
}

/** A titled row of cards that scrolls sideways. */
export function CardRow(props: { title: string; items: Item[]; class?: string }) {
  return (
    <View class={`gap-2 ${props.class ?? ""}`}>
      <SectionTitle class="px-3">{props.title}</SectionTitle>
      <Scroll horizontal scrollbar={false}>
        <View class="flex-row">
          <Each of={props.items} key={(i) => i.uri}>
            {(item) => <Card item={item()} />}
          </Each>
        </View>
      </Scroll>
    </View>
  );
}

/** Cards that wrap into a grid (an artist's releases). */
export function CardGrid(props: { items: Item[] }) {
  return (
    <View class="flex-row flex-wrap">
      <Each of={props.items} key={(i) => i.uri}>
        {(item) => <Card item={item()} />}
      </Each>
    </View>
  );
}

/** The header row above a track list. */
export function TrackHeader(props: { showAlbum: () => boolean }) {
  return (
    <View class="mx-3 mb-2 h-9 flex-row items-center gap-4 border-b border-border px-4">
      <Text class="w-6 text-right text-xs font-medium text-muted-foreground">#</Text>
      <Text class="flex-1 text-xs font-medium text-muted-foreground">Title</Text>
      <If when={props.showAlbum()}>
        <Text class="flex-1 text-xs font-medium text-muted-foreground">Album</Text>
      </If>
      <View class="w-12 items-end text-muted-foreground">
        <Icon name="clock" size={16} color="muted-foreground" />
      </View>
    </View>
  );
}

/** One track row (used in `List`, so read from `track()` each render). */
export function TrackRow(props: {
  track: () => Track | undefined;
  number: () => number;
  showAlbum: () => boolean;
  onPress: () => void;
}) {
  const t = () => props.track();
  const current = () => {
    const uri = t()?.uri ?? "";
    return uri !== "" && store.player().trackUri === uri;
  };
  const playing = () => current() && store.player().playing;
  return (
    <Button
      accessibilityLabel={t() ? `${t()!.name} ${t()!.artists}` : ""}
      onPress={props.onPress}
      class="mx-3 h-14 flex-row items-center gap-4 rounded-(--radius) px-4 hover:bg-row-hover active:opacity-75 focus:outline focus:outline-2 focus:outline-ring"
      classList={{ "opacity-45": !!t() && !t()!.playable }}
    >
      <View class="w-6 items-end">
        <If
          when={current()}
          else={<Text class="text-sm text-muted-foreground tabular-nums">{props.number() + 1}</Text>}
        >
          <View class="text-primary">
            <Icon name={playing() ? "volume" : "play"} size={14} color="primary" />
          </View>
        </If>
      </View>
      <Cover src={() => t()?.image ?? ""} size={40} fallback="music" class="rounded" />
      <View class="min-w-0 flex-1 gap-0.5">
        <Text class={`line-clamp-1 ${current() ? "text-base text-primary" : "text-base text-foreground"}`}>
          {t()?.name ?? ""}
        </Text>
        <View class="flex-row items-center gap-1.5">
          <If when={!!t() && t()!.explicit}>
            <Text class="rounded-sm bg-muted-foreground px-1 text-[9px] font-bold text-background">E</Text>
          </If>
          <Text class="line-clamp-1 flex-1 text-sm text-muted-foreground">{t()?.artists ?? ""}</Text>
        </View>
      </View>
      <If when={props.showAlbum()}>
        <Text class="line-clamp-1 min-w-0 flex-1 text-sm text-muted-foreground">{t()?.album ?? ""}</Text>
      </If>
      <Text class="w-12 text-right text-sm text-muted-foreground tabular-nums">
        {t() ? clock(t()!.durationMs) : ""}
      </Text>
    </Button>
  );
}

/** A page's message in place of content: loading, empty, an error. */
export function PageMessage(props: { title: string; detail?: string; class?: string }) {
  return (
    <View class={`items-center gap-2 px-6 pt-16 ${props.class ?? ""}`}>
      <Text class="text-lg font-bold text-foreground">{props.title}</Text>
      <If when={props.detail}>
        <Text class="max-w-[420px] text-center text-sm text-muted-foreground">{props.detail}</Text>
      </If>
    </View>
  );
}

export { List };
