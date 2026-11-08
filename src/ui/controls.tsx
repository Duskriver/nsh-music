// The app's styled controls, each wrapping a base component once. Screens
// compose these; they do not restyle base controls themselves.

import type { JSX } from "nsh";
import { Image, View } from "nsh";
import { Button, Slider } from "nsh/base";
import { Icon, type IconName } from "./icons";

/** A borderless icon button: muted, brighter on hover, primary while `on`. */
export function IconButton(props: {
  name: () => IconName;
  label: string;
  size?: number;
  on?: () => boolean;
  disabled?: () => boolean;
  onPress: () => void;
}) {
  const size = () => props.size ?? 18;
  const color = () => (props.on?.() ? "primary" : "muted-foreground");
  return (
    <Button
      accessibilityLabel={props.label}
      disabled={props.disabled?.() ?? false}
      onPress={props.onPress}
      class="items-center justify-center rounded-full hover:opacity-85 active:opacity-60 active:scale-95 disabled:opacity-35 focus:outline focus:outline-2 focus:outline-ring"
      style={{ width: size() + 12, height: size() + 12 }}
    >
      <Icon name={props.name()} size={size()} color={color()} />
    </Button>
  );
}

/** The round play / pause button, `size` across. */
export function PlayButton(props: {
  playing: () => boolean;
  size?: number;
  primary?: boolean;
  disabled?: () => boolean;
  onPress: () => void;
}) {
  const size = () => props.size ?? 36;
  const primary = () => props.primary ?? false;
  return (
    <Button
      accessibilityLabel={props.playing() ? "Pause" : "Play"}
      disabled={props.disabled?.() ?? false}
      onPress={props.onPress}
      class={`items-center justify-center rounded-full hover:scale-105 active:scale-95 active:opacity-85 disabled:opacity-40 focus:outline focus:outline-2 focus:outline-ring ${primary() ? "bg-primary" : "bg-control"}`}
      style={{ width: size(), height: size() }}
    >
      <Icon
        name={props.playing() ? "pause" : "play"}
        size={size() * 0.42}
        color={primary() ? "primary-foreground" : "control-foreground"}
      />
    </Button>
  );
}

/** A pill button with a label: `primary` filled, else outlined. */
export function PillButton(props: { label: string; primary?: boolean; onPress: () => void }) {
  const primary = () => props.primary ?? false;
  return (
    <Button
      onPress={props.onPress}
      class={`h-9 items-center justify-center rounded-full px-5 hover:scale-105 active:opacity-70 focus:outline focus:outline-2 focus:outline-ring ${
        primary() ? "bg-foreground" : "border border-muted-foreground"
      }`}
    >
      <TextLabel bold class={primary() ? "text-background text-sm" : "text-foreground text-sm"}>
        {props.label}
      </TextLabel>
    </Button>
  );
}

// A tiny text helper so controls don't each import Text; screens use <Text> directly.
import { Text } from "nsh";
function TextLabel(props: { bold?: boolean; class?: string; children: JSX.Element }) {
  return <Text class={`${props.bold ? "font-bold" : ""} ${props.class ?? ""}`}>{props.children}</Text>;
}

/** An item's image, square (or round for an artist), with a glyph fallback. */
export function Cover(props: {
  src: () => string;
  size?: number;
  round?: boolean;
  fallback?: IconName;
  class?: string;
}) {
  const round = () => props.round ?? false;
  return (
    <View
      class={`shrink-0 items-center justify-center overflow-hidden bg-secondary text-muted-foreground ${
        round() ? "rounded-full" : "rounded-md"
      } ${props.size ? "" : "aspect-square w-full"} ${props.class ?? ""}`}
      style={props.size ? { width: props.size, height: props.size } : {}}
    >
      <Icon name={props.fallback ?? "music"} size={props.size ? Math.max(16, props.size * 0.4) : 40} color="muted-foreground" />
      <If when={props.src()}>
        <Image
          src={props.src()}
          style={{ position: "absolute", top: 0, left: 0, right: 0, bottom: 0 }}
          class="object-cover"
        />
      </If>
    </View>
  );
}

/** A thin bar slider (progress, volume): range in the text color, thumb on hover. */
export function BarSlider(props: {
  label: string;
  value: () => number;
  max: () => number;
  disabled?: () => boolean;
  class?: string;
  onChange: (v: number) => void;
  onChangeEnd: (v: number) => void;
}) {
  return (
    <Slider
      accessibilityLabel={props.label}
      value={props.value()}
      min={0}
      max={props.max()}
      step={0}
      disabled={props.disabled?.() ?? false}
      class={props.class}
      trackClass="h-4 justify-center group"
      indicatorClass="h-1 rounded-full bg-muted"
      rangeClass="rounded-full bg-foreground group-hover:bg-primary"
      thumbClass="size-3 rounded-full bg-foreground opacity-0 group-hover:opacity-100 active:opacity-100 focus:opacity-100"
      onChange={({ value }) => typeof value === "number" && props.onChange(value)}
      onChangeEnd={({ value }) => typeof value === "number" && props.onChangeEnd(value)}
    />
  );
}

/** A section's title. */
export function SectionTitle(props: { children: JSX.Element; class?: string }) {
  return (
    <Text role="header" class={`text-xl font-bold text-foreground ${props.class ?? ""}`}>
      {props.children}
    </Text>
  );
}
