// A page's header on the desktops, the web and in a terminal (iOS and
// Android show the system's, from the page's `title`): Back and Forward,
// and the account's action.

import { createMotion, signal, Text, token, untracked, View } from "nsh";
import { Input } from "nsh/base";
import { back, canBack, canForward, currentRoute, forward, open, SEARCH } from "../nav";
import { DISTANCE, DURATION, EASING } from "./ui/tokens";
import { query } from "../state/search";
import { Icon } from "./ui/icons";
import { openPremium } from "../state/links";
import { player } from "../state/player";
import { signedIn, signIn } from "../state/session";
import { layout, sidebarShown } from "./layout";
import { Account } from "./library/sidebar";
import { closeMainWindow, hasAppWindow } from "../platform/desktop";
import { sidebarCollapsed, toggleSidebar } from "../state/sidebar";
import { ActionButton, IconButton, terminal } from "./ui/controls";

/** Log in while signed out; Premium for a free account; nothing
 * while the session is not known yet. */
export function AccountAction() {
  return (
    <View class="flex-row items-center gap-2">
      <If when={signedIn() === true && player().premium === false}>
        {/* An icon at every width, beside the header's other icons. */}
        <IconButton name="crown" label="Premium" hint onPress={openPremium} />
      </If>
      <If when={signedIn() === false}>
        <ActionButton label="Log in" variant="primary" onPress={signIn} />
      </If>
    </View>
  );
}

/** The desktop's search, in the header's top-right corner: a Search
 * button that opens into a field. Return searches (opens Search with what
 * was typed); on the search page the field stays open and the results
 * follow the typing. Leaving it empty (it loses the focus, or Escape)
 * folds it back. */
export function SearchField() {
  const onSearch = () => currentRoute().name === "search";
  const expanded = signal(onSearch() && query().trim() !== "");
  return (
    <If when={expanded() || onSearch()} else={<IconButton name="search" label="Search" hint onPress={() => expanded.set(true)} />}>
      <SearchBox
        onSearch={onSearch()}
        onDone={() => expanded.set(false)}
      />
    </If>
  );
}

/** The opened field: it grows out from the right, focused. */
function SearchBox(props: { onSearch: boolean; onDone: () => void }) {
  const draft = signal(untracked(() => query()));
  const grow = untracked(() =>
    terminal
      ? undefined
      : createMotion({
          enter: {
            keyframes: { opacity: [0, 1], scaleX: [0.7, 1], translateX: [DISTANCE.large, 0] },
            duration: DURATION.base,
            easing: EASING.out,
          },
        }).style,
  );
  const submit = (text: string) => {
    const term = text.trim();
    if (!term) return;
    query.set(text);
    if (!props.onSearch) open(SEARCH);
  };
  return (
    <View
      class="h-10 w-72 flex-row items-center gap-2 rounded-full border border-secondary bg-secondary px-3.5 transition duration-150 ease-out focus-within:border-foreground/40 motion-reduce:transition-none"
      style={grow}
    >
      <Icon name="search" size={16} color="muted-foreground" />
      <Input
        value={props.onSearch ? query() : draft()}
        onChange={({ value }) => (props.onSearch ? query.set(value) : draft.set(value))}
        onSubmit={({ text }) => submit(text)}
        onBlur={() => {
          if (!(props.onSearch ? query() : draft()).trim()) props.onDone();
        }}
        onKeyPress={({ key }) => {
          if (key === "Escape" && !props.onSearch) props.onDone();
        }}
        placeholder="What do you want to play?"
        accessibilityLabel="Search Spotify"
        returnKeyType="search"
        submitBehavior="submit"
        autoFocus
        class="flex-1 text-sm text-foreground"
        style={{ placeholderColor: token("muted-foreground"), caretColor: token("foreground") }}
      />
    </View>
  );
}

/** The header of a page: Back (and Forward where there is room), the
 * page's title when `title` shows, and the account's action. */
export function PageHeader(props: { title: string; showTitle?: boolean }) {
  const desktop = () => layout() === "desktop";
  return (
    <View
      class={
        terminal
          ? "flex-row items-center gap-1"
          : "min-h-14 flex-row items-center gap-2 px-4 pt-[env(safe-area-inset-top)]"
      }
    >
      {/* With the sidebar folded away, the account comes up here: the
          user's picture, which opens the user menu. */}
      <If when={desktop() && !terminal && !sidebarShown()}>
        <Account compact placement="bottom" />
      </If>
      {/* Folds the sidebar away entirely, or brings it back. */}
      <If when={desktop() && !terminal}>
        <IconButton
          name={sidebarCollapsed() ? "sidebarExpand" : "sidebarCollapse"}
          label={sidebarCollapsed() ? "Expand sidebar" : "Collapse sidebar"}
          hint
          onPress={toggleSidebar}
        />
      </If>
      <If when={canBack() || desktop()}>
        <IconButton name="chevronLeft" label="Go back" short="Back" disabled={!canBack()} onPress={back} />
      </If>
      <If when={desktop()}>
        <IconButton name="chevronRight" label="Go forward" short="Forward" disabled={!canForward()} onPress={forward} />
      </If>
      <If when={props.showTitle ?? false}>
        <Text role="header" class="line-clamp-1 flex-1 text-lg font-bold text-foreground">
          {props.title}
        </Text>
      </If>
      <View class="flex-1" />
      <If when={desktop() && !terminal}>
        <SearchField />
      </If>
      <AccountAction />
      {/* Closes the main window, as the title bar's close would: it hides
          while the tray keeps the app (the music plays on). */}
      <If when={desktop() && !terminal && hasAppWindow}>
        <IconButton name="close" label="Close window" hint onPress={closeMainWindow} />
      </If>
    </View>
  );
}
