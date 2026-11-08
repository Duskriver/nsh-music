// Now Playing: the cover large, what plays and the full transport, in a
// full-screen sheet over the app (Escape, back and the down chevron close it).

import { cleanup, createMotion, effect, View, windowSize } from "nsh";
import { coverColorsQuery } from "../../data/queries";
import { nowPlayingOpen, setNowPlayingOpen } from "../../state/overlays";
import { player } from "../../state/player";
import { lightEffect } from "../../state/visual";
import { LightMenu } from "./light-menu";
import { Cover, IconButton } from "../ui/controls";
import { AppSheet } from "../ui/overlays";
import { titleBarArea } from "../ui/title-bar";
import { MusicLight } from "../ui/toolbar-fx";
import { animateLayoutOn, enter } from "../ui/motion";
import { LikeButton, NEW_COVER, NowPlayingSummary, SeekBar, TransportControls, VolumeControl } from "./parts";

function NowPlayingContent() {
  // A new track's cover settles in, as in the bar.
  const coverIn = createMotion();
  let shown = player().artwork;
  effect(() => {
    const now = player().artwork;
    if (now !== shown) void coverIn.play(NEW_COVER);
    shown = now;
  });
  cleanup(() => coverIn.stop());

  // The same colour as the desktop's player: the cover's.
  const colors = coverColorsQuery(() => player().artwork);

  // Responsive: the cover takes what room the window leaves. Tall, it sits
  // above the rest; wide and short, beside it; with too little room for
  // either, it gives way and the words and controls stay.
  const side = () => windowSize().width >= 640 && windowSize().height < 560;
  const coverSize = () => {
    const { width, height } = windowSize();
    // Room the top row, the words, the position, the controls and the gaps
    // take (stacked), or the top row and the margins (beside).
    const size = side() ? Math.min(height - 120, width * 0.38) : Math.min(width - 48, height - 400);
    const fitted = Math.min(320, Math.floor(size));
    return fitted < 96 ? 0 : fitted;
  };
  // Rearranging (above / beside / no cover), it glides there.
  animateLayoutOn(() => `${side()}|${coverSize() > 0}`);
  const cover = () => (
    <View style={coverIn.style}>
      <Cover src={player().artwork} size={coverSize()} class="rounded-2xl shadow-xl" />
    </View>
  );
  const details = () => (
    <>
      <View class="flex-row items-center gap-4" style={enter("fadeInUp", 140)}>
        <NowPlayingSummary large cover={0} />
        <LikeButton size={24} />
      </View>
      <View style={enter("fadeInUp", 200)}>
        <SeekBar quiet />
      </View>
      <View class="items-center" style={enter("fadeInUp", 260)}>
        <TransportControls large quiet />
      </View>
    </>
  );

  return (
    <View class="flex-1 bg-sidebar pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)]">
      {/* The music's light rises from the bottom of the page. */}
      <MusicLight effect={lightEffect()} colors={colors()} playing={player().playing} reach={windowSize().height * 0.45} />
      {/* The top row starts under the window controls (the band the
          system's title bar had, which drags the window), its close button
          under the traffic lights' left end: clear of them, hover
          included, without a band of its own above it. */}
      <View
        class="z-10 flex-row items-center justify-between px-3"
        desktop={{ windowDrag: true }}
        style={{ paddingTop: titleBarArea().height > 0 ? titleBarArea().height : 16 }}
      >
        <IconButton name="chevronDown" label="Close Now Playing" short="Back" size={24} onPress={() => setNowPlayingOpen(false)} />
        {/* The corner: the volume (always open here, nothing to reveal) and
            the light. */}
        <View class="flex-row items-center gap-1">
          <VolumeControl quiet />
          <LightMenu placement="bottom" size={22} />
        </View>
      </View>
      <If
        when={side()}
        else={
          <View class="min-h-0 flex-1 items-center justify-center gap-6 px-6 pb-8">
            {/* Opening, in layers: the cover grows in, then the words, the
                position and the controls rise one after another. */}
            <If when={coverSize() > 0}>
              <View style={enter("zoomIn", 60)}>{cover()}</View>
            </If>
            {/* The light leads here: the controls are quiet (thin, faint). */}
            <View class="w-full max-w-160 gap-6">{details()}</View>
          </View>
        }
      >
        {/* Wide and short: the cover beside the rest. */}
        <View class="min-h-0 flex-1 flex-row items-center justify-center gap-8 px-8 pb-6">
          <View style={enter("zoomIn", 60)}>{cover()}</View>
          <View class="min-w-0 max-w-140 flex-1 gap-5">{details()}</View>
        </View>
      </If>
    </View>
  );
}

/** Now Playing, open while the overlay store says so. */
export function NowPlayingSheet() {
  return (
    <AppSheet
      open={nowPlayingOpen()}
      onOpenChange={setNowPlayingOpen}
      side="bottom"
      full
      closeOnSwipe
      testID="now-playing"
    >
      <NowPlayingContent />
    </AppSheet>
  );
}
