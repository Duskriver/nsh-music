// The window's layout: a desktop workspace (sidebar + main + player bar) that
// becomes a phone layout (full-width pages, a bottom tab bar, a mini player) as
// the window narrows. Over both sit Now Playing, About, the sign-in prompt and
// notices.

import { mount, platform, Scroll, Text, View, windowSize } from "nsh";
import { keymap } from "nsh";
import { store } from "./src/ui/store";
import { IconButton, PillButton } from "./src/ui/controls";
import { Icon } from "./src/ui/icons";
import { Home } from "./src/screens/home";
import { Search } from "./src/screens/search";
import { Collection } from "./src/screens/collection";
import { NowPlaying } from "./src/screens/nowplaying";
import { Sidebar } from "./src/screens/sidebar";
import { PlayerBar, MiniPlayer } from "./src/screens/playerbar";
import { About } from "./src/screens/about";

const NARROW = 560;
const phone = () => !platform.desktop || windowSize().width < NARROW;

function CurrentScreen() {
  return (
    <View class="min-h-0 flex-1">
      <If when={store.route().name === "home"}><Home /></If>
      <If when={store.route().name === "search"}><Search /></If>
      <If when={store.route().name === "collection"}>
        <Collection uri={(store.route() as { uri: string }).uri} />
      </If>
    </View>
  );
}

function TopBar(props: { wide: boolean }) {
  return (
    <View class="absolute left-0 right-0 top-0 h-16 flex-row items-center gap-2 px-4">
      <If when={props.wide}>
        <IconButton name={() => "chevronLeft"} label="Go back" size={18} disabled={() => !store.canBack()} onPress={() => store.back()} />
        <IconButton name={() => "chevronRight"} label="Go forward" size={18} disabled={() => !store.canForward()} onPress={() => store.forward()} />
      </If>
      <View class="flex-1" />
      <If when={store.signedIn() === true && !store.player().premium}>
        <PillButton label="Explore Premium" onPress={() => store.openPremium()} />
      </If>
      <If when={store.signedIn() === false}>
        <PillButton label="Log in" primary onPress={() => store.signIn()} />
      </If>
    </View>
  );
}

function StatusLine() {
  const message = () => {
    const h = store.health();
    if (h.status === "starting") return { text: "Starting the Spotify web player…", bad: false };
    if (h.status === "failed") return { text: h.error ?? "The player failed to start.", bad: true };
    return null;
  };
  return (
    <If when={message()}>
      {(m) => (
        <View class={`absolute left-4 right-4 top-16 flex-row items-center gap-2 rounded-(--radius) bg-popover px-4 py-2 shadow-md ${m().bad ? "text-destructive" : "text-muted-foreground"}`}>
          <Icon name="alert" size={16} color={m().bad ? "destructive" : "muted-foreground"} />
          <Text class="flex-1 text-sm">{m().text}</Text>
        </View>
      )}
    </If>
  );
}

function MainView(props: { wide: boolean }) {
  return (
    <View class={`min-w-0 flex-1 overflow-hidden bg-background ${props.wide ? "rounded-(--radius-panel)" : ""}`}>
      <CurrentScreen />
      <TopBar wide={props.wide} />
      <StatusLine />
    </View>
  );
}

function BottomTab(props: { name: "home" | "search" | "library" }) {
  const selected = () =>
    props.name === "library" ? store.drawer() : !store.drawer() && store.route().name === props.name;
  const icon = props.name === "home" ? "home" : props.name === "search" ? "search" : "library";
  const label = props.name === "home" ? "Home" : props.name === "search" ? "Search" : "Library";
  return (
    <View
      role="tab"
      accessibilityLabel={label}
      onPress={() => (props.name === "library" ? store.drawer.update((d) => !d) : store.openAndCloseDrawer({ name: props.name } as never))}
      class={`flex-1 items-center justify-center gap-1 py-2 active:opacity-60 ${selected() ? "text-foreground" : "text-muted-foreground"}`}
    >
      <Icon name={icon} size={24} color={selected() ? "foreground" : "muted-foreground"} />
      <Text class="text-xs font-medium">{label}</Text>
    </View>
  );
}

function Phone() {
  return (
    <View class="flex-1">
      <MainView wide={false} />
      <MiniPlayer />
      <View class="flex-row items-stretch border-t border-border bg-player px-2 pt-1 pb-[env(safe-area-inset-bottom)]">
        <BottomTab name="home" />
        <BottomTab name="search" />
        <BottomTab name="library" />
      </View>
    </View>
  );
}

function Drawer() {
  return (
    <If when={phone() && store.drawer()}>
      <View class="absolute inset-0 flex-row bg-shell/60" onPress={() => store.drawer.set(false)}>
        <View class="w-[320px] max-w-[320px] flex-1 bg-shell p-2" onPress={() => {}}>
          <Sidebar />
        </View>
      </View>
    </If>
  );
}

function Desktop() {
  return (
    <View class="flex-1">
      <View class="min-h-0 flex-1 flex-row gap-2 p-2 pb-0">
        <Sidebar />
        <MainView wide />
      </View>
      <PlayerBar />
    </View>
  );
}

function Notice() {
  return (
    <If when={store.notice()}>
      {(text) => (
        <View class="absolute bottom-[96px] left-0 right-0 items-center">
          <Text class="rounded-(--radius) bg-foreground px-4 py-2 text-sm font-medium text-background shadow-lg">{text()}</Text>
        </View>
      )}
    </If>
  );
}

function SigningIn() {
  return (
    <If when={store.health().status === "signingIn"}>
      <View class="absolute inset-0 items-center justify-center bg-shell/85">
        <View class="w-[400px] max-w-[400px] items-center gap-4 rounded-(--radius-panel) bg-popover p-8 shadow-2xl">
          <View class="text-primary"><Icon name="logIn" size={36} color="primary" /></View>
          <Text class="text-xl font-bold text-foreground">Log in to Spotify</Text>
          <Text class="text-center text-sm text-muted-foreground">
            A Spotify window has opened. Log in there — your account, or Google, Apple or Facebook. When your library
            loads, <Text class="font-semibold text-foreground">close that window</Text> to finish; Spotish saves the
            login and takes over playback.
          </Text>
          <View class="flex-row gap-3">
            <PillButton label="Cancel" onPress={() => store.cancelSignIn()} />
          </View>
        </View>
      </View>
    </If>
  );
}

function NowPlayingOverlay() {
  return (
    <If when={store.nowPlaying()}>
      <View class="absolute inset-0 bg-background">
        <NowPlaying />
      </View>
    </If>
  );
}

function App() {
  keymap.bind("space", () => store.toggle());
  keymap.bind("mod+right", () => store.control("next", () => store.spotify.next()));
  keymap.bind("mod+left", () => store.control("previous", () => store.spotify.previous()));
  keymap.bind("escape", () => {
    if (store.nowPlaying()) store.nowPlaying.set(false);
    else if (store.about()) store.about.set(false);
    else if (store.drawer()) store.drawer.set(false);
  });
  return (
    <View class="flex-1 bg-shell">
      <If when={phone()} else={<Desktop />}><Phone /></If>
      <Drawer />
      <NowPlayingOverlay />
      <About />
      <SigningIn />
      <Notice />
    </View>
  );
}

mount(App);
