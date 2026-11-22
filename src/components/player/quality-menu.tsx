// Audio quality selector menu, shared by the desktop player bar, Now Playing
// and the terminal player. Choosing a tier updates the engine, reloads the
// current track at the new quality, and is remembered between runs (`state/player`).

import { Text, View, signal } from "nsh";
import { Button, Menu, MenuItem } from "nsh/base";
import type { AudioQuality } from "../../backend/types";
import { player, setQuality } from "../../state/player";
import { ready } from "../../state/session";
import { focusRing, Hint, LAYER, menuItem, popupSurface, terminal } from "../ui/controls";
import { Icon } from "../ui/icons";
import { menuPopupHidden, menuPopupLeaving, menuPopupStyle } from "../ui/menu";
import { regionItem } from "../ui/regions";

interface QualityOption {
  id: AudioQuality;
  badge: string;
  label: string;
  detail: string;
}

const QUALITIES: QualityOption[] = [
  { id: "standard", badge: "STD", label: "Standard", detail: "128k" },
  { id: "exhigh", badge: "HQ", label: "High", detail: "320k" },
  { id: "lossless", badge: "SQ", label: "Lossless", detail: "FLAC" },
  { id: "hires", badge: "HR", label: "Hi-Res", detail: "FLAC" },
];

export function QualityMenu(props: { placement: "top" | "bottom" }) {
  const open = signal(false);
  const current = () => QUALITIES.find((q) => q.id === player().quality) ?? QUALITIES[1];
  const label = () => `Audio quality: ${current().label}`;

  return (
    <Menu
      open={open()}
      onOpenChange={(e) => open.set(e.open)}
      placement={props.placement}
      align="end"
      offset={6}
      popupClass={`w-56 ${popupSurface}`}
      popupStyle={menuPopupStyle(props.placement, "end")}
      popupStartingStyle={menuPopupHidden(props.placement, "end")}
      popupEndingStyle={menuPopupLeaving(props.placement, "end")}
      trigger={
        <Button
          ref={regionItem()}
          accessibilityLabel={label()}
          selected={open()}
          disabled={!ready()}
          tooltip={open() ? undefined : <Hint label={`${label()} (${current().detail})`} />}
          class={
            terminal
              ? "px-1 focus:bg-muted"
              : `group relative h-7 items-center justify-center rounded-full px-2.5 disabled:opacity-40 ${focusRing}`
          }
        >
          <If when={terminal}>
            <Text class="text-foreground">[{current().badge}]</Text>
          </If>
          <If when={!terminal}>
            <View
              class={`absolute inset-0 rounded-full bg-foreground/10 opacity-0 scale-75 ${LAYER} group-hover:scale-100 group-hover:opacity-100 group-selected:scale-100 group-selected:opacity-100 group-active:scale-95`}
            />
            <View class={`relative flex-row items-center rounded-full border border-foreground/15 px-1.5 py-0.5 ${LAYER} opacity-75 group-hover:border-foreground/30 group-hover:opacity-100 group-selected:border-foreground/30 group-selected:opacity-100 group-active:scale-95`}>
              <Text class="font-mono text-[11px] font-semibold tracking-tight text-foreground">{current().badge}</Text>
            </View>
          </If>
        </Button>
      }
    >
      <Each of={QUALITIES} key={(q) => q.id}>
        {(q) => (
          <MenuItem
            role="menuitemradio"
            checked={player().quality === q().id}
            onSelect={() => setQuality(q().id)}
            class={terminal ? "px-1 focus:bg-muted" : `group ${menuItem} gap-2`}
            indicatorClass={terminal ? undefined : "hidden"}
          >
            <Text class="w-8 font-mono text-xs text-muted-foreground">{q().badge}</Text>
            <Text class="line-clamp-1 flex-1 text-sm text-popover-foreground group-checked:font-semibold">{q().label}</Text>
            <Text class="text-xs text-muted-foreground">{q().detail}</Text>
            <View class="w-4 items-end">
              <If when={!terminal && player().quality === q().id}>
                <Icon name="check" size={12} color="foreground" />
              </If>
            </View>
          </MenuItem>
        )}
      </Each>
    </Menu>
  );
}
