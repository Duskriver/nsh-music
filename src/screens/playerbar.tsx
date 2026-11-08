// The bottom player bar: what plays, the transport controls and the volume.
// A compact MiniPlayer stands in for it on phones.

import { Text, View } from "nsh";
import { store } from "../ui/store";
import { clock } from "../ui/types";
import { BarSlider, Cover, IconButton, PlayButton } from "../ui/controls";
import type { IconName } from "../ui/icons";

// Section 1: the cover, title and artist of what plays, plus the like button.
function NowPlaying() {
  return (
    <View class="w-[300px] min-w-0 flex-row items-center gap-3">
      <View
        role="button"
        accessibilityLabel="Open the now playing view"
        class="rounded-md hover:opacity-85 active:scale-95"
        onPress={() => {
          if (store.player().title) store.nowPlaying.set(true);
        }}
      >
        <Cover src={() => store.player().artwork} size={56} />
      </View>
      <View class="min-w-0 flex-1 gap-0.5">
        <Text class="line-clamp-1 text-sm font-semibold text-foreground">
          {store.player().title || "Nothing playing"}
        </Text>
        <View class="flex-row items-center gap-2">
          <Text class="line-clamp-1 text-xs text-muted-foreground">
            {store.player().ad ? "Advertisement" : store.player().artist}
          </Text>
          <If when={store.player().ad}>
            <Text
              role="button"
              accessibilityLabel="Get Premium"
              class="text-xs font-bold text-primary hover:opacity-85 active:opacity-60"
              onPress={() => store.openPremium()}
            >
              Get Premium
            </Text>
          </If>
        </View>
      </View>
      <IconButton
        name={() => (store.player().liked ? "heartFilled" : "heart")}
        label={store.player().liked ? "Remove from Liked Songs" : "Save to Liked Songs"}
        size={18}
        on={() => store.player().liked}
        disabled={() => store.player().title === "" || store.player().ad}
        onPress={() => store.control("like", () => store.spotify.like())}
      />
    </View>
  );
}

// Section 2: the transport controls over the playback position.
function Transport() {
  const position = () => store.seeking() ?? store.player().positionMs;
  const duration = () => Math.max(1, store.player().durationMs);
  const hasTrack = () => store.player().title !== "";
  return (
    <View class="flex-1 max-w-[720px] items-center gap-1">
      <View class="flex-row items-center gap-4">
        <IconButton
          name={() => "shuffle"}
          label="Shuffle"
          size={18}
          on={() => store.player().shuffle}
          disabled={() => !store.player().shuffleAvailable}
          onPress={() => store.control("shuffle", () => store.spotify.shuffle())}
        />
        <IconButton
          name={() => "skipBack"}
          label="Previous"
          size={18}
          disabled={() => !store.player().canPrevious}
          onPress={() => store.control("previous", () => store.spotify.previous())}
        />
        <PlayButton playing={() => store.player().playing} size={36} onPress={() => store.toggle()} />
        <IconButton
          name={() => "skipForward"}
          label="Next"
          size={18}
          disabled={() => !store.player().canNext}
          onPress={() => store.control("next", () => store.spotify.next())}
        />
        <IconButton
          name={() => (store.player().repeat === "track" ? "repeatOne" : "repeat")}
          label={
            store.player().repeat === "off"
              ? "Repeat"
              : store.player().repeat === "context"
                ? "Repeat one"
                : "Don't repeat"
          }
          size={18}
          on={() => store.player().repeat !== "off"}
          onPress={() => store.control("repeat", () => store.spotify.repeat())}
        />
      </View>
      <View class="w-full flex-row items-center gap-2">
        <Text class="w-10 text-right text-xs tabular-nums text-muted-foreground">
          {clock(position())}
        </Text>
        <BarSlider
          label="Playback position"
          value={() => store.seeking() ?? store.player().positionMs}
          max={() => Math.max(1, store.player().durationMs)}
          disabled={() => store.player().title === ""}
          class="flex-1"
          onChange={(v) => store.seeking.set(v)}
          onChangeEnd={(v) => {
            store.seek(v);
            store.seeking.set(null);
          }}
        />
        <Text class="w-10 text-xs tabular-nums text-muted-foreground">
          {clock(hasTrack() ? duration() : 0)}
        </Text>
      </View>
    </View>
  );
}

// Section 3: mute and the volume slider.
function Volume() {
  const level = () => store.volumeDraft() ?? (store.player().muted ? 0 : store.player().volume);
  const volumeIcon = (): IconName => {
    const v = level();
    if (v <= 0) return "volumeMute";
    if (v < 0.5) return "volumeLow";
    return "volume";
  };
  return (
    <View class="w-[300px] flex-row items-center justify-end gap-1">
      <IconButton
        name={volumeIcon}
        label={store.player().muted ? "Unmute" : "Mute"}
        size={18}
        onPress={() => store.control("mute", () => store.spotify.mute())}
      />
      <BarSlider
        label="Volume"
        value={() => store.volumeDraft() ?? (store.player().muted ? 0 : store.player().volume)}
        max={() => 1}
        class="w-[110px]"
        onChange={(v) => store.volumeDraft.set(v)}
        onChangeEnd={(v) => {
          store.setVolume(v);
          store.volumeDraft.set(null);
        }}
      />
    </View>
  );
}

/** The full desktop player bar. */
export function PlayerBar() {
  return (
    <View class="h-[76px] shrink-0 flex-row items-center justify-between gap-4 px-2 bg-player">
      <NowPlaying />
      <Transport />
      <Volume />
    </View>
  );
}

/** A compact player for phones; hidden when nothing is playing. */
export function MiniPlayer() {
  return (
    <If when={store.player().title !== ""}>
      <View class="mx-2 mb-2 h-14 flex-row items-center gap-3 rounded-(--radius-card) bg-card px-2">
        <View
          role="button"
          accessibilityLabel="Open the now playing view"
          class="min-w-0 flex-1 flex-row items-center gap-3"
          onPress={() => store.nowPlaying.set(true)}
        >
          <Cover src={() => store.player().artwork} size={44} />
          <View class="min-w-0 flex-1 gap-0.5">
            <Text class="line-clamp-1 text-sm font-semibold text-foreground">
              {store.player().title}
            </Text>
            <Text class="line-clamp-1 text-xs text-muted-foreground">
              {store.player().ad ? "Advertisement" : store.player().artist}
            </Text>
          </View>
        </View>
        <PlayButton playing={() => store.player().playing} size={36} onPress={() => store.toggle()} />
      </View>
    </If>
  );
}
