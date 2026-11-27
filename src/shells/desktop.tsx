// The desktop's shell: the sidebar (a rail on narrow windows) down the left,
// and the pages on the right with the player floating over their bottom, a
// pill the pages scroll behind.

import { Animated, cleanup, type CompositeAnimation, createAnimatedValue, effect, reduceMotion, signal, untracked, View } from "nsh";
import { Toolbar } from "nsh/base";
import { Region } from "../components/ui/regions";
import { barSize, openSidebarWidth, railSidebar, sidebarShown } from "../components/layout";
import { animateLayoutOn } from "../components/ui/motion";
import { DURATION, EASING, SPRING } from "../components/ui/tokens";
import { Sidebar } from "../components/library/sidebar";
import { PlayerBar } from "../components/player/player-bar";
import { MainStack } from "./main";
import { nowPlayingOpen } from "../state/overlays";
import { TitleBar, titleBarArea } from "../components/ui/title-bar";
import { NavControls, TrailingControls } from "../components/header";

const SHELL_GAP = 8;

export function DesktopShell() {
  // Crossing a breakpoint while the sidebar arrangement stays the same,
  // panels glide to their new arrangement. Collapse/expand drives the
  // sidebar on the compositor alone so pages with wide text rows don't
  // re-measure and re-rasterize on every animation frame.
  let bpShown = untracked(sidebarShown);
  let lastBp = `${railSidebar()}|${barSize()}`;
  let bpGen = 0;
  animateLayoutOn(() => {
    const shown = sidebarShown();
    const bp = `${railSidebar()}|${barSize()}`;
    if (shown !== bpShown) {
      bpShown = shown;
      lastBp = bp;
    } else if (bp !== lastBp) {
      lastBp = bp;
      bpGen++;
    }
    return String(bpGen);
  });

  const initialShown = untracked(sidebarShown);
  const slide = createAnimatedValue(initialShown ? 0 : -(untracked(openSidebarWidth) + SHELL_GAP));
  const visible = signal(initialShown);
  let running: CompositeAnimation | null = null;
  let lastShown = initialShown;
  let gen = 0;

  effect(() => {
    const shown = sidebarShown();
    if (shown === lastShown) return;
    lastShown = shown;
    running?.stop();
    running = null;
    const token = ++gen;
    const offX = -(untracked(openSidebarWidth) + SHELL_GAP);
    if (shown) {
      const wasVisible = untracked(visible);
      visible.set(true);
      if (reduceMotion()) {
        slide.set(0);
        return;
      }
      if (!wasVisible) slide.set(offX);
      running = Animated.spring(slide, { toValue: 0, ...SPRING.snappy });
      void running.start(({ finished }) => {
        if (finished && token === gen) running = null;
      });
    } else {
      if (reduceMotion()) {
        slide.set(offX);
        visible.set(false);
        return;
      }
      running = Animated.timing(slide, { toValue: offX, duration: DURATION.base, easing: EASING.out });
      void running.start(({ finished }) => {
        if (finished && token === gen) {
          running = null;
          visible.set(false);
        }
      });
    }
  });
  cleanup(() => {
    gen++;
    running?.stop();
  });

  return (
    <View class="flex-1">
      {/* The header's controls, after the name: one stop of Tab, Left and
          Right between its buttons (the search field is a stop of its own). */}
      <TitleBar>
        <Region.Provider value={{ name: "header" }}>
          <Toolbar accessibilityLabel="Header" class="flex-1 items-center gap-2">
            <NavControls />
            <View class="flex-1" />
            <TrailingControls />
          </Toolbar>
        </Region.Provider>
      </TitleBar>
      <View class="flex-1 px-2 pb-2" style={{ paddingTop: titleBarArea().height > 0 ? 0 : 8 }}>
        <View class="relative flex-1 overflow-hidden">
          <View
            accessibilityElementsHidden={!sidebarShown()}
            class="bg-sidebar"
            style={{
              position: "absolute",
              left: 0,
              top: 0,
              bottom: 0,
              zIndex: 10,
              width: openSidebarWidth(),
              pointerEvents: sidebarShown() ? "auto" : "none",
              visibility: visible() ? "visible" : "hidden",
              transform: [{ translateX: slide }],
            }}
          >
            <Sidebar compact={railSidebar()} />
          </View>
          <View
            class="relative min-w-0 flex-1"
            style={{ marginLeft: sidebarShown() ? openSidebarWidth() + SHELL_GAP : 0 }}
          >
            <MainStack class="min-w-0 overflow-hidden" />
            {/* Not drawn under Now Playing, which covers it. */}
            <If when={!nowPlayingOpen()}>
              <View class="absolute inset-x-3 bottom-1">
                <PlayerBar />
              </View>
            </If>
          </View>
        </View>
      </View>
    </View>
  );
}
