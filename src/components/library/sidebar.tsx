// The sidebar: the main pages, the user's library and the account. On a
// narrow desktop window it is a `compact` rail of icons and covers (each
// with a tooltip); the phone's drawer shows it in full.

import { hasAppWindow, quitApp } from "../../platform/desktop";
import { type AnimatedValue, createAnimatedValue, createMotion, effect, fpsEnabled, List, reduceMotion, setFpsEnabled, signal, Text, untracked, View } from "nsh";
import { Button, Menu, MenuItem, MenuSeparator } from "nsh/base";
import type { Item } from "../../backend/types";
import { libraryQuery, meQuery } from "../../data/queries";
import { currentRoute, type Route, switchTo } from "../../nav";
import { NSH_URL, openExternal, openPremium, REPO_URL, SPOTIFY_URL } from "../../state/links";
import { setAboutOpen, setDrawerOpen } from "../../state/overlays";
import { isCurrentContext, isPlayingContext, player } from "../../state/player";
import { signedIn, signIn, signOut } from "../../state/session";
import { ActionButton, Cover, focusRing, Hint, IconButton, LAYER, menuItem, popupSurface, terminal } from "../ui/controls";
import { ErrorState, LibrarySkeleton, Reveal } from "../ui/feedback";
import { Icon, iconMotion, type IconName } from "../ui/icons";
import { PlayingBars } from "../ui/playing";
import { createListWave, listItemEntrance, type ListWave, WAVE } from "../ui/motion";
import { DURATION, EASING } from "../ui/tokens";
import { menuPopupHidden, menuPopupLeaving, menuPopupStyle } from "../ui/menu";
import { fallbackIcon } from "./cards";
import { accountMenuWidth, layout } from "../layout";

/** Switches to a page from the sidebar, cross-fading rather than
 * sliding (and closes the phone's drawer). */
function go(route: Route) {
  switchTo(route);
  setDrawerOpen(false);
}

/** A sidebar row's height and the gap under it (h-11, and gap-1 or pb-1):
 * rows are 48pt apart in both panels. */
const ROW_STEP = 44 + 4;

/** A panel's selection (transitions.dev "Tabs sliding"): one background
 * behind the rows that slides to the selected one (`index`, -1: none), rather
 * than each row lighting its own. With no selection it fades away; from no
 * selection it fades in where it lands. `top`: the first row's top in the
 * panel; `scroll`: the offset of the list it sits behind, which it follows
 * natively, without easing (a row scrolled away goes out of the panel, which
 * clips it). */
function SlidingSelection(props: { index: () => number; top: number; scroll?: AnimatedValue }) {
  const motion = createMotion({ initial: { opacity: 0 } });
  // Where it was last sent: undefined before the first placement, -1 while
  // hidden.
  let at: number | undefined;
  effect(() => {
    const i = props.index();
    untracked(() => {
      const from = at;
      at = i;
      if (from === i) return;
      const y = props.top + i * ROW_STEP;
      // The first placement, and reduced motion, jump; later ones slide.
      if (from === undefined || reduceMotion()) {
        if (i >= 0) motion.translateY.set(y);
        motion.opacity.set(i >= 0 ? 1 : 0);
        return;
      }
      const timing = { duration: DURATION.base, easing: EASING.inOut };
      if (i < 0) {
        void motion.opacity.timing(0, timing);
        return;
      }
      if (from < 0) motion.translateY.set(y);
      else void motion.translateY.timing(y, timing);
      void motion.opacity.timing(1, timing);
    });
  });
  const y = props.scroll ? motion.translateY.sub(props.scroll) : motion.translateY;
  return (
    <View
      class="pointer-events-none absolute left-2 right-2 top-0 h-11 rounded-(--radius-inset) bg-secondary"
      style={{ opacity: motion.opacity, transform: [{ translateY: y }] }}
    />
  );
}

/** The nav panel's selection: Home (and Search, where the panel lists it). */
function NavIndicator() {
  const index = () => {
    const name = currentRoute().name;
    if (name === "home") return 0;
    // The desktop lists Search in the header, not here.
    if (name === "search" && layout() !== "desktop") return 1;
    return -1;
  };
  return <SlidingSelection index={index} top={8} />;
}

/** One of the main pages. */
function NavItem(props: { icon: IconName; label: string; route: Route; compact: boolean }) {
  const selected = () => currentRoute().name === props.route.name;
  const hover = iconMotion();
  return (
    <View onMouseEnter={() => hover.play(props.icon)}>
    <Button
      accessibilityLabel={props.label}
      selected={selected()}
      onPress={() => go(props.route)}
      tooltip={props.compact ? <Hint label={props.label} /> : undefined}
      class={
        terminal
          ? "px-1 focus:bg-muted selected:bg-muted"
          : `group h-11 flex-row items-center gap-3 rounded-(--radius-inset) transition duration-200 ease-out hover:bg-row-hover active:scale-[0.98] motion-reduce:transition-none ${focusRing} ${
              props.compact ? "justify-center" : "px-2"
            }`
      }
    >
      <If when={!terminal}>
        {/* As wide as a library row's cover, so icons, covers and labels
            line up down the whole sidebar. */}
        <View class={`w-[30px] items-center ${LAYER} group-active:scale-90`}>
          <View style={hover.style}>
            <Icon name={props.icon} size={22} color={selected() ? "foreground" : "muted-foreground"} />
          </View>
        </View>
      </If>
      <If when={!props.compact || terminal}>
        <Text class={`text-base font-bold ${LAYER} group-hover:translate-x-0.5 ${selected() ? "text-foreground" : "text-muted-foreground group-hover:text-foreground"}`}>{props.label}</Text>
      </If>
    </Button>
    </View>
  );
}

/** One library entry: a cell of the recycling list, so it reads `item()`
 * where it is used. */
function LibraryRow(props: { item: Item | undefined; compact: boolean; index: number; wave?: ListWave }) {
  // The rows wave in when the library shows; a row scrolled to comes in on
  // its own (its cover flipping open just after). Made once, here: the
  // Button re-reads its style with its other props, and a motion made there
  // would play again.
  const { style: entrance, cover: coverIn } = untracked(() =>
    listItemEntrance({
      wave: props.wave,
      key: () => props.item?.uri ?? "",
      index: () => props.index,
      axis: "translateX",
      turns: WAVE.rows,
    }),
  );
  const uri = () => props.item?.uri ?? "";
  const kind = () => props.item?.kind ?? "";
  const selected = () => {
    const route = currentRoute();
    return route.name === "collection" && route.uri === uri();
  };
  const playingHere = () => isPlayingContext(uri());
  const current = () => isCurrentContext(uri());
  return (
    <Button
      accessibilityLabel={props.item?.name ?? ""}
      selected={selected()}
      onPress={() => uri() && go({ name: "collection", uri: uri() })}
      tooltip={props.compact ? <Hint label={props.item?.name ?? ""} /> : undefined}
      style={entrance}
      class={
        terminal
          ? "flex-row gap-1 px-1 focus:bg-muted selected:bg-muted"
          : `group h-11 flex-row items-center gap-3 rounded-(--radius-inset) px-2 transition duration-200 ease-out hover:bg-row-hover active:scale-[0.98] active:bg-muted motion-reduce:transition-none ${focusRing} ${
              props.compact ? "justify-center" : ""
            }`
      }
    >
      <View style={coverIn}>
        <Cover
          src={props.item?.image ?? ""}
          size={props.compact ? 36 : 30}
          round
          liked={kind() === "collection"}
          fallback={fallbackIcon(kind())}
          class={`${LAYER} group-hover:scale-110 group-active:scale-95`}
        />
      </View>
      <If when={!props.compact || terminal}>
        <Text class={`line-clamp-1 min-w-0 flex-1 text-sm font-medium ${current() ? "text-primary" : "text-foreground"}`}>
          {props.item?.name ?? ""}
        </Text>
        <If when={current() && !terminal}>
          <PlayingBars playing={playingHere()} size={12} />
        </If>
      </If>
    </Button>
  );
}

/** The library: the list (signed in), a login prompt (signed out), or
 * skeleton rows while it is not known yet. */
function LibraryPanel(props: { compact: boolean }) {
  const library = libraryQuery();
  // The list's scroll offset, for the selection behind its rows.
  const scroll = createAnimatedValue(0);
  // Which row is selected: the open collection's place in the library.
  const selectedIndex = () => {
    const route = currentRoute();
    if (route.name !== "collection") return -1;
    return library.data()?.items.findIndex((i) => i.uri === route.uri) ?? -1;
  };
  return (
    <View accessibilityLabel="Your Library" class={terminal ? "min-h-0 flex-1" : "relative min-h-0 flex-1 overflow-hidden rounded-(--radius-panel) border border-border bg-card"}>
      <If when={!terminal && signedIn() !== false}>
        {/* Behind the rows (the list's own top padding is 8pt). */}
        <SlidingSelection index={selectedIndex} top={8} scroll={scroll} />
      </If>
      <If when={signedIn() !== false} else={<SignedOutPrompt compact={props.compact} />}>
        <Reveal
          data={library.data()}
          placeholder={<LibraryPlaceholder query={library} compact={props.compact} />}
          class="min-h-0 flex-1"
        >
          {(lib, instant) => {
            // The rows wave in when they first show (at once when cached,
            // as the skeleton fades when they loaded); rows scrolled to come
            // in on their own.
            const wave = createListWave({ isTop: () => true, afterReveal: !instant });
            return (
              <List
                each={lib().items}
                keyExtractor={(i) => i.uri}
                estimatedItemSize={48}
                // Stops with a whole row at the top (rows are 48pt apart).
                snapToInterval={48}
                snapToAlignment="start"
                scrollValue={scroll}
                contentClass="px-2 pt-2 pb-1"
                class="flex-1"
              >
                {(item, index) => (
                  <View class="pb-1">
                    <LibraryRow item={item()} compact={props.compact} index={index()} wave={wave} />
                  </View>
                )}
              </List>
            );
          }}
        </Reveal>
      </If>
      {/* Rows leave the panel softly at its bottom edge, not cut off. */}
      <If when={!terminal}>
        <View class="pointer-events-none absolute inset-x-0 bottom-0 h-8 bg-gradient-to-t from-card to-transparent" />
      </If>
    </View>
  );
}

function LibraryPlaceholder(props: { query: ReturnType<typeof libraryQuery>; compact: boolean }) {
  return (
    <If when={props.query.error() && !props.compact} else={<LibrarySkeleton compact={props.compact} rows={props.compact ? 4 : 6} />}>
      <ErrorState message="Your library didn't load." onRetry={props.query.refetch} />
    </If>
  );
}

function SignedOutPrompt(props: { compact: boolean }) {
  return (
    <If
      when={props.compact && !terminal}
      else={
        <View class="m-2 gap-3 rounded-(--radius-card) border border-border p-4">
          <Text class="text-base font-bold text-foreground">Your playlists live here</Text>
          <Text class="text-sm text-muted-foreground">Log in with your Spotify account, free or Premium, to see your library.</Text>
          <View class="flex-row">
            <ActionButton label="Log in" variant="primary" onPress={signIn} />
          </View>
        </View>
      }
    >
      <View class="m-2 items-center">
        <IconButton name="logIn" label="Log in" hint onPress={signIn} />
      </View>
    </If>
  );
}

/** One row of the user menu: its icon, centred in a slot as wide as the
 * avatar above (so the icons line up under its centre and the labels under
 * the name), the label, and a mark for a page that opens in the browser,
 * or, for a setting (`checked` given), a check while it is on. */
function Entry(props: { icon: IconName; label: string; external?: boolean; checked?: boolean; onSelect: () => void }) {
  const hover = iconMotion();
  return (
    <MenuItem onSelect={props.onSelect} checked={props.checked} class={terminal ? "px-1 focus:bg-muted" : menuItem}>
      <View onMouseEnter={() => hover.play(props.icon)} class="flex-1 flex-row items-center gap-2.5">
        <If when={!terminal}>
          <View class="w-8 items-center" style={hover.style}>
            <Icon name={props.icon} size={16} color="muted-foreground" />
          </View>
        </If>
        <Text class="flex-1 text-sm text-popover-foreground">{props.label}</Text>
        <If when={props.external && !terminal}>
          <Icon name="externalLink" size={14} color="muted-foreground" />
        </If>
        <If when={props.checked && !terminal}>
          <Icon name="check" size={14} color="popover-foreground" />
        </If>
      </View>
    </MenuItem>
  );
}

const Separator = () => <MenuSeparator class={terminal ? "hidden" : "-mx-1 my-1 h-px bg-border"} />;

/** The user menu: who is signed in and on which plan, then Premium, Spotish
 * and NSH, the FPS monitor, and Log out. */
function MenuEntries(props: { name: string; image: string }) {
  const premium = () => player().premium === true;
  return (
    <>
      <If when={signedIn() === true && !terminal}>
        <View class="flex-row items-center gap-2.5 px-2.5 py-2">
          <Cover src={props.image} size={32} round fallback="user" />
          <View class="min-w-0 flex-1">
            <Text class="line-clamp-1 text-sm font-semibold text-popover-foreground">{props.name}</Text>
            <Text class="text-xs text-muted-foreground">{premium() ? "Spotify Premium" : "Spotify Free"}</Text>
          </View>
        </View>
        <Separator />
      </If>
      <If when={!premium()}>
        <Entry icon="crown" label="Premium" external onSelect={openPremium} />
      </If>
      <Entry icon="music" label="Spotify" external onSelect={() => openExternal(SPOTIFY_URL)} />
      <Separator />
      <Entry icon="info" label="About…" onSelect={() => setAboutOpen(true)} />
      <Entry icon="github" label="GitHub" external onSelect={() => openExternal(REPO_URL)} />
      <Entry icon="code" label="NSH" external onSelect={() => openExternal(NSH_URL)} />
      {/* NSH's performance monitor: frame rate, frame and commit costs. */}
      <Entry icon="activity" label="FPS" checked={fpsEnabled()} onSelect={() => setFpsEnabled(!fpsEnabled())} />
      <If when={signedIn() === true || hasAppWindow}>
        <Separator />
      </If>
      <If when={signedIn() === true}>
        <Entry icon="logOut" label="Log out" onSelect={() => void signOut()} />
      </If>
      {/* Ends the app (the music stops), as the tray's Exit. */}
      <If when={hasAppWindow}>
        <Entry icon="power" label="Exit" onSelect={quitApp} />
      </If>
    </>
  );
}


/** The account at the sidebar's foot, which opens the user menu: the
 * user's picture and name (Spotish, signed out). */
export function Account(props: { compact: boolean; placement?: "top" | "bottom"; align?: "start" | "end" }) {
  const me = meQuery();
  // Up from the sidebar's foot; down from the header's corner.
  const side = () => props.placement ?? "top";
  // Flush with its left edge, or (in the window's top-right corner) its right.
  const align = () => props.align ?? "start";
  const name = () => (signedIn() === true ? me.data()?.name || "Account" : "Spotish");
  const open = signal(false);
  return (
    <Menu
      open={open()}
      onOpenChange={(e) => open.set(e.open)}
      placement={side()}
      align={align()}
      offset={4}
      popupClass={popupSurface}
      // As wide as the account row, so it opens flush with it.
      popupStyle={{ ...menuPopupStyle(side(), align()), width: accountMenuWidth() }}
      popupStartingStyle={menuPopupHidden(side(), align())}
      popupEndingStyle={menuPopupLeaving(side(), align())}
      trigger={
        <Button
          accessibilityLabel={`User menu: ${name()}`}
          selected={open()}
          tooltip={props.compact ? <Hint label={name()} /> : undefined}
          class={
            terminal
              ? "px-1 focus:bg-muted"
              : `group h-11 flex-row items-center gap-3 rounded-(--radius-inset) transition duration-200 ease-out hover:bg-row-hover active:scale-[0.98] selected:bg-secondary motion-reduce:transition-none ${focusRing} ${
                  props.compact ? `justify-center ${side() === "bottom" ? "w-11 rounded-full" : ""}` : "px-2"
                }`
          }
        >
          <Cover
            src={signedIn() === true ? (me.data()?.image ?? "") : ""}
            size={30}
            round
            fallback="user"
            class={LAYER}
          />
          <If when={!props.compact || terminal}>
            <Text class="line-clamp-1 flex-1 text-sm font-semibold text-foreground">{name()}</Text>
            <If when={!terminal}>
              {/* The chevron points the way the menu opens. */}
              <View class={`${side() === "top" ? "rotate-180" : ""} ${LAYER} opacity-60 group-hover:opacity-100 group-hover:-translate-y-0.5`}>
                <Icon name="chevronDown" size={14} color="muted-foreground" />
              </View>
            </If>
          </If>
        </Button>
      }
    >
      <MenuEntries name={name()} image={me.data()?.image ?? ""} />
    </Menu>
  );
}

/** The whole sidebar; `compact` is the desktop's icon rail. */
export function Sidebar(props: { compact?: boolean }) {
  const compact = () => (props.compact ?? false) && !terminal;
  return (
    <View
      role="navigation"
      class={terminal ? "min-h-0 flex-1 gap-1" : "min-h-0 flex-1 gap-2"}
    >
      <View class={terminal ? "" : `relative gap-1 rounded-(--radius-panel) border border-border bg-card p-2`}>
        <If when={!terminal}>
          <NavIndicator />
        </If>
        <NavItem icon="home" label="Home" route={{ name: "home" }} compact={compact()} />
        {/* The desktop searches from the header's top-right corner; the
            phone's drawer and the terminal keep Search here. */}
        <If when={layout() !== "desktop" || terminal}>
          <NavItem icon="search" label="Search" route={{ name: "search" }} compact={compact()} />
        </If>
      </View>
      <LibraryPanel compact={compact()} />
      {/* The account: a panel of its own, its row like every other. */}
      <View class={terminal ? "" : "rounded-(--radius-panel) border border-border bg-card p-2"}>
        <Account compact={compact()} />
      </View>
    </View>
  );
}
