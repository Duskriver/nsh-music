// The full-screen Now Playing view: an aurora tinted by the track, the big
// album art, and the transport. It is NSH showing a live shader, a floating
// animation and native layout behind one coherent Spotify-style screen.

import { Animated, createAnimatedValue, effect, Text, View } from "nsh";
import { store } from "../ui/store";
import { clock } from "../ui/types";
import { Ambient, colorsFor } from "../ui/shader";
import { BarSlider, Cover, IconButton, PlayButton } from "../ui/controls";

export function NowPlaying() {
  // The art breathes while playing: a slow drift up and down, paused at rest.
  const float = createAnimatedValue(0);
  let loop: ReturnType<typeof Animated.loop> | null = null;
  effect(() => {
    if (store.player().playing) {
      if (!loop) {
        loop = Animated.loop(
          Animated.sequence([
            Animated.timing(float, { toValue: 1, duration: 2600, easing: "easeInOut" }),
            Animated.timing(float, { toValue: 0, duration: 2600, easing: "easeInOut" }),
          ]),
        );
        loop.start();
      }
    } else if (loop) {
      loop.stop();
      loop = null;
      float.set(0);
    }
  });

  const position = () => store.seeking() ?? store.player().positionMs;
  const duration = () => Math.max(1, store.player().durationMs);
  const hasTrack = () => store.player().title !== "";

  return (
    <Ambient colors={() => colorsFor(store.player().trackUri)}>
      {/* The close chevron, over the aurora. */}
      <View class="absolute top-4 left-4 z-10">
        <IconButton
          name={() => "chevronDown"}
          label="Close"
          size={24}
          onPress={() => store.nowPlaying.set(false)}
        />
      </View>

      <View class="flex-1 items-center justify-center gap-8 p-8">
        {/* The album art, lifted on a soft shadow, floating a little. */}
        <View
          style={{
            transform: [
              { translateY: float.interpolate({ inputRange: [0, 1], outputRange: [0, -10] }) },
            ],
          }}
        >
          <Cover src={() => store.player().artwork} size={360} class="rounded-2xl shadow-2xl" />
        </View>

        <View class="w-full max-w-[520px] gap-6">
          <View class="flex-row items-center gap-4">
            <View class="min-w-0 flex-1 gap-1">
              <Text class="line-clamp-2 text-3xl font-extrabold text-foreground">
                {store.player().title || "Nothing playing"}
              </Text>
              <Text class="line-clamp-1 text-base text-muted-foreground">
                {store.player().ad ? "Advertisement" : store.player().artist}
              </Text>
            </View>
            <IconButton
              name={() => (store.player().liked ? "heartFilled" : "heart")}
              label={store.player().liked ? "Remove from Liked Songs" : "Save to Liked Songs"}
              size={24}
              on={() => store.player().liked}
              disabled={() => store.player().title === "" || store.player().ad}
              onPress={() => store.control("like", () => store.spotify.like())}
            />
          </View>

          <View class="gap-1">
            <BarSlider
              label="Playback position"
              value={() => store.seeking() ?? store.player().positionMs}
              max={() => Math.max(1, store.player().durationMs)}
              disabled={() => store.player().title === ""}
              onChange={(v) => store.seeking.set(v)}
              onChangeEnd={(v) => {
                store.seek(v);
                store.seeking.set(null);
              }}
            />
            <View class="flex-row justify-between">
              <Text class="text-xs tabular-nums text-muted-foreground">{clock(position())}</Text>
              <Text class="text-xs tabular-nums text-muted-foreground">
                {clock(hasTrack() ? duration() : 0)}
              </Text>
            </View>
          </View>

          <View class="flex-row items-center justify-center gap-6">
            <IconButton
              name={() => "shuffle"}
              label="Shuffle"
              size={22}
              on={() => store.player().shuffle}
              disabled={() => !store.player().shuffleAvailable}
              onPress={() => store.control("shuffle", () => store.spotify.shuffle())}
            />
            <IconButton
              name={() => "skipBack"}
              label="Previous"
              size={26}
              disabled={() => !store.player().canPrevious}
              onPress={() => store.control("previous", () => store.spotify.previous())}
            />
            <PlayButton playing={() => store.player().playing} size={68} onPress={() => store.toggle()} />
            <IconButton
              name={() => "skipForward"}
              label="Next"
              size={26}
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
              size={22}
              on={() => store.player().repeat !== "off"}
              onPress={() => store.control("repeat", () => store.spotify.repeat())}
            />
          </View>
        </View>
      </View>
    </Ambient>
  );
}
