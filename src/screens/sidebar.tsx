// The sidebar: the main pages, the user's library and the account. On narrower
// desktop windows it collapses to an icon rail — just the icons and library
// covers, no labels. Ported from the Rust `components/sidebar.rs`.

import { List, resource, Text, View, windowSize } from "nsh";
import { Button } from "nsh/base";
import { store, type Route } from "../ui/store";
import { Cover, IconButton, PillButton } from "../ui/controls";
import { Icon, type IconName } from "../ui/icons";
import type { Item, Library, Me } from "../ui/types";

/** Whether the sidebar is an icon rail (a narrow desktop window). */
const rail = () => windowSize().width < 1100;

/** The glyph a library item falls back to when it has no image. */
function fallbackIcon(kind: string): IconName {
  if (kind === "artist") return "user";
  if (kind === "album") return "disc";
  if (kind === "collection") return "heartFilled";
  return "music";
}

/** One of the two top-nav entries: an icon and (off the rail) a bold label. */
function NavItem(props: { icon: IconName; label: string; route: Route }) {
  const selected = () => store.route().name === props.route.name;
  return (
    <Button
      accessibilityLabel={props.label}
      onPress={() => store.openAndCloseDrawer(props.route)}
      class="h-10 flex-row items-center gap-4 rounded-(--radius) hover:opacity-100 active:opacity-70 focus:outline focus:outline-2 focus:outline-ring"
      classList={{ "justify-center": rail(), "px-3": !rail() }}
    >
      <Icon name={props.icon} size={24} color={selected() ? "foreground" : "muted-foreground"} />
      <If when={!rail()}>
        <Text class={`text-base font-bold ${selected() ? "text-foreground" : "text-muted-foreground"}`}>
          {props.label}
        </Text>
      </If>
    </Button>
  );
}

/** One library entry in the virtualized list (read `item()` each render). */
function LibraryRow(props: { item: () => Item | undefined }) {
  const item = () => props.item();
  const kind = () => item()?.kind ?? "";
  const selected = () => {
    const it = item();
    const r = store.route();
    return !!it && r.name === "collection" && r.uri === it.uri;
  };
  const playingHere = () => {
    const it = item();
    if (!it) return false;
    const id = it.uri.split(":").pop() ?? "";
    const p = store.player();
    return p.playing && id !== "" && p.contextHref.endsWith(id);
  };
  return (
    <Button
      accessibilityLabel={item()?.name ?? ""}
      onPress={() => {
        const it = item();
        if (it) store.openAndCloseDrawer({ name: "collection", uri: it.uri });
      }}
      class="flex-row items-center gap-3 rounded-(--radius) p-2 hover:bg-sidebar-accent active:opacity-75 focus:outline focus:outline-2 focus:outline-ring"
      classList={{ "justify-center": rail(), "bg-sidebar-accent": selected() }}
    >
      <Cover src={() => item()?.image ?? ""} size={48} round={kind() === "artist"} fallback={fallbackIcon(kind())} />
      <If when={!rail()}>
        <View class="min-w-0 flex-1 gap-0.5">
          <Text class={`line-clamp-1 text-base ${playingHere() ? "text-primary" : "text-foreground"}`}>
            {item()?.name ?? ""}
          </Text>
          <Text class="line-clamp-1 text-sm text-muted-foreground">{item()?.subtitle ?? ""}</Text>
        </View>
      </If>
      <If when={playingHere() && !rail()}>
        <Icon name="volume" size={16} color="primary" />
      </If>
    </Button>
  );
}

/** The library panel: header, then the list (signed in) or a login prompt. */
function LibraryPanel() {
  const lib = resource(store.epoch, () => store.spotify.library() as Promise<Library>);
  return (
    <View class="min-h-0 flex-1 rounded-(--radius-panel) bg-sidebar">
      <View
        class="h-14 flex-row items-center gap-3 text-muted-foreground"
        classList={{ "justify-center": rail(), "px-5": !rail() }}
      >
        <Icon name="library" size={24} color="muted-foreground" />
        <If when={!rail()}>
          <Text class="text-base font-bold text-muted-foreground">Your Library</Text>
        </If>
      </View>
      <If
        when={store.signedIn() === true}
        else={
          <If
            when={rail()}
            else={
              <View class="m-2 gap-3 rounded-(--radius-card) bg-secondary p-4">
                <Text class="text-base font-bold text-foreground">Your playlists live here</Text>
                <Text class="text-sm text-muted-foreground">
                  Log in with your Spotify account, free or Premium, to see your library and play music.
                </Text>
                <View class="flex-row">
                  <PillButton label="Log in" primary onPress={() => store.signIn()} />
                </View>
              </View>
            }
          >
            <View class="m-2 items-start">
              <IconButton name={() => "logIn"} label="Log in" size={20} onPress={() => store.signIn()} />
            </View>
          </If>
        }
      >
        <List
          each={lib()?.items ?? []}
          keyExtractor={(i) => i.uri}
          estimatedItemSize={64}
          contentClass="p-2"
          class="flex-1"
          empty={() => (
            <Text class="px-5 text-sm text-muted-foreground" classList={{ hidden: rail() }}>
              {lib.error() ? `Your library didn't load: ${String(lib.error())}` : "Loading your library…"}
            </Text>
          )}
        >
          {(item) => <LibraryRow item={item} />}
        </List>
      </If>
    </View>
  );
}

/** The account strip: the signed-in user's avatar, name and a log-out button. */
function Account() {
  const me = resource(store.epoch, () => store.spotify.me() as Promise<Me>);
  return (
    <If when={store.signedIn() === true}>
      <View
        class="h-14 flex-row items-center gap-3 rounded-(--radius-panel) bg-sidebar"
        classList={{ "justify-center": rail(), "px-4": !rail() }}
      >
        <Cover src={() => me()?.image ?? ""} size={30} round fallback="user" />
        <If when={!rail()}>
          <Text class="line-clamp-1 flex-1 text-sm font-semibold text-foreground">
            {me()?.name || "Signed in"}
          </Text>
          <IconButton name={() => "logOut"} label="Log out" size={18} onPress={() => store.signOut()} />
        </If>
      </View>
    </If>
  );
}

/** A small text link in the footer. */
function FooterLink(props: { label: string; onPress: () => void }) {
  return (
    <Text
      role="button"
      onPress={props.onPress}
      class="text-xs font-medium text-muted-foreground hover:text-foreground active:opacity-60"
    >
      {props.label}
    </Text>
  );
}

/** The footer of text links (hidden on the rail). */
function Footer() {
  return (
    <If when={!rail()}>
      <View class="gap-2 px-4 py-3">
        <View class="flex-row flex-wrap gap-x-4 gap-y-1">
          <FooterLink label="Get Premium" onPress={() => store.openPremium()} />
          <FooterLink label="spotify.com" onPress={() => store.openExternal("https://www.spotify.com/")} />
          <FooterLink label="About" onPress={() => store.about.set(true)} />
        </View>
        <Text
          role="button"
          onPress={() => store.openExternal("https://nsh.dev")}
          class="text-xs text-muted-foreground hover:text-foreground"
        >
          Built with NSH — native UI in Rust
        </Text>
      </View>
    </If>
  );
}

export function Sidebar() {
  return (
    <View class="shrink-0 gap-2 text-sidebar-foreground" style={{ width: rail() ? 72 : 300 }}>
      <View class="rounded-(--radius-panel) bg-sidebar" classList={{ "p-1": rail(), "p-2": !rail() }}>
        <NavItem icon="home" label="Home" route={{ name: "home" }} />
        <NavItem icon="search" label="Search" route={{ name: "search" }} />
      </View>
      <LibraryPanel />
      <Account />
      <Footer />
    </View>
  );
}
