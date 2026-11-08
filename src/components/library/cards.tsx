// Cards: playlists, albums, artists and tracks as tiles, in rows that
// scroll sideways or in a wrapping grid.

import { List, signal, Text, untracked, View } from "nsh";
import { Button } from "nsh/base";
import { type Item, opens } from "../../backend/types";
import { open } from "../../nav";
import { isPlayingContext, playItem, toggleCollection } from "../../state/player";
import { ready } from "../../state/session";
import { Cover, focusRing, PlayButton, SectionTitle, terminal } from "../ui/controls";
import type { IconName } from "../ui/icons";
import { listItemEntrance, type ListWave, WAVE } from "../ui/motion";

/** The glyph an item shows while it has no image. */
export function fallbackIcon(kind: string): IconName {
  if (kind === "artist") return "user";
  if (kind === "album") return "disc";
  if (kind === "collection") return "heartFilled";
  return "music";
}

/** Opens an item's page, or plays it when it has none (a track). */
export function activate(item: Item): void {
  if (opens(item)) open({ name: "collection", uri: item.uri });
  else playItem(item);
}

// The play button over a card's cover, in golden proportion: the cover
// (the card less its 10pt padding each side; 124pt in a 144pt card) is φ³
// times the button, and the button φ² times its inset from the cover's
// corner (29 and 11 in a 144pt card).
const PHI = (1 + Math.sqrt(5)) / 2;
/** A card's width where nothing sizes it (a row), pt. */
export const CARD_WIDTH = 144;
const playSize = (card: number) => Math.round((card - 2 * 10) / PHI ** 3);
const playInset = (card: number) => Math.round(playSize(card) / PHI ** 2);

/** One card: picture, name, and what it is. With its row's `wave`, it rises
 * in after the cards before it (`index`) each time the page is entered, and
 * on its own when it is scrolled into view. On hover it shows its play
 * button. */
export function Card(props: {
  item: Item;
  index?: number;
  wave?: ListWave;
  /** About how wide it is drawn, pt: what its play button is sized by. */
  width?: number;
  /** Stretched by its container (a grid's cell) rather than `width` wide. */
  fill?: boolean;
}) {
  const width = () => props.width ?? CARD_WIDTH;
  const playing = () => isPlayingContext(props.item.uri);
  const play = () => (opens(props.item) ? toggleCollection(props.item.uri) : playItem(props.item));
  // Made once, here: inside the Button's style it would be made (and
  // played) again whenever the Button's props re-run. Its cover flips open
  // just after the card starts to rise.
  const { style: rise, cover: coverIn } = untracked(() =>
    listItemEntrance({
      wave: props.wave,
      key: () => props.item?.uri ?? "",
      index: () => props.index ?? 0,
      axis: "translateY",
      turns: WAVE.cards,
    }),
  );
  return (
    <Button
      accessibilityLabel={`${props.item.name}, ${props.item.subtitle}`}
      onPress={() => activate(props.item)}
      style={terminal || props.fill ? rise : { ...rise, width: width() }}
      class={
        terminal
          ? "px-1 focus:bg-muted"
          : `group shrink-0 gap-2 rounded-(--radius-card) p-2.5 transition duration-200 ease-out hover:bg-row-hover active:scale-[0.98] motion-reduce:transition-none motion-reduce:active:scale-100 ${focusRing}`
      }
    >
      <View class="relative">
        {/* Layered on hover, and steady: the card doesn't move; a wash comes
            in, the cover grows a hair (1%), the play button rises over it. */}
        <View class="transition duration-300 ease-out group-hover:scale-[1.01] motion-reduce:transition-none motion-reduce:group-hover:scale-100">
          <View style={coverIn}>
            <Cover
              src={props.item.image}
              round={props.item.kind === "artist"}
              liked={props.item.kind === "collection"}
              fallback={fallbackIcon(props.item.kind)}
              class="shadow-md"
            />
          </View>
        </View>
        <If when={!terminal}>
          <View
            style={{ right: playInset(width()), bottom: playInset(width()) }}
            class={`absolute transition duration-200 ease-out group-hover:translate-y-0 group-hover:scale-100 group-hover:opacity-100 motion-reduce:transition-none ${
              playing() ? "translate-y-0 scale-100 opacity-100" : "translate-y-3 scale-75 opacity-0"
            }`}
          >
            <PlayButton playing={playing()} size={playSize(width())} disabled={!ready()} onPress={play} />
          </View>
        </If>
      </View>
      <View class={terminal ? "flex-row gap-1" : "gap-1"}>
        <Text class="line-clamp-1 text-sm font-semibold text-foreground">{props.item.name}</Text>
        <Text class="line-clamp-2 text-xs text-muted-foreground">{props.item.subtitle}</Text>
      </View>
    </Button>
  );
}

/** A titled row of cards that scrolls sideways (a list in a terminal).
 * With the page's `wave`, its cards come in as a wave of their own, `delay`
 * ms after the page's, each time the page is entered; cards scrolled to
 * come in on their own. */
export function CardRow(props: { title: string; items: Item[]; class?: string; wave?: ListWave; delay?: number }) {
  const wave = untracked(() => props.wave?.child(props.delay ?? 0));
  return (
    <View class={`gap-2 ${props.class ?? ""}`}>
      <SectionTitle class="px-3">{props.title}</SectionTitle>
      <If
        when={!terminal}
        else={
          <View>
            <Each of={props.items} key={(i) => i.uri}>
              {(item, index) => <Card item={item()} index={index()} wave={wave} />}
            </Each>
          </View>
        }
      >
        {/* Virtualized: only the cards near the view exist, recycled as the
            row scrolls (a row can hold dozens, each with its cover). The row
            is as tall as a card with two lines under it, plus room for the
            hover lift. */}
        <List
          horizontal
          each={props.items}
          keyExtractor={(i) => i.uri}
          estimatedItemSize={144}
          // Scrolling stops with a card's start at the gutter (cards are
          // 144pt apart), never halfway through one.
          snapToInterval={144}
          snapToAlignment="start"
          decelerationRate="fast"
          scrollbar={false}
          // The row runs out to the page's edges and insets its cards by the
          // page's gutter (12pt), so a snapped card stops 12pt in, never
          // flush against or cut by the edge, and the first lines up with
          // the title.
          class="-mx-3 h-[216px]"
          contentClass="px-3 pt-1"
        >
          {(item, index) => (
            <If when={item()}>{(it) => <Card item={it()} index={index()} wave={wave} />}</If>
          )}
        </List>
      </If>
    </View>
  );
}

/** Cards that wrap into a grid (an artist's releases); with the page's
 * `wave` they come in as a wave of their own, like a row's. */
export function CardGrid(props: { items: Item[]; wave?: ListWave; delay?: number }) {
  const wave = untracked(() => props.wave?.child(props.delay ?? 0));
  // As many columns as fit at a card's width, widened to share the row out
  // so no room is left at its end. The grid lays the columns out: cards
  // given widths that add up to the row wrap the last one early once
  // layout rounds them to the screen's pixels. The width a card gets is
  // still worked out here, for its play button.
  const room = signal(0);
  const cardWidth = () => {
    const w = room();
    return w > 0 ? w / Math.max(1, Math.floor(w / CARD_WIDTH)) : CARD_WIDTH;
  };
  return (
    <View
      class={terminal ? "" : "grid"}
      style={terminal ? undefined : { gridTemplateColumns: `repeat(auto-fill, minmax(${CARD_WIDTH}px, 1fr))` }}
      onLayout={(e) => room.set(e.bounds.size.width)}
    >
      <Each of={props.items} key={(i) => i.uri}>
        {(item, index) => (
          <Card item={item()} index={index()} wave={wave} width={terminal ? undefined : cardWidth()} fill={!terminal} />
        )}
      </Each>
    </View>
  );
}
