// The control above a playlist's tracks that picks their order and view.

import { signal, Text, View } from "nsh";
import { Button, Menu, MenuItem, MenuSeparator } from "nsh/base";
import { t } from "../../i18n";
import { focusRing, menuItem, popupSurface, terminal } from "../ui/controls";
import { Icon, type IconColor } from "../ui/icons";
import { menuPopupHidden, menuPopupLeaving, menuPopupStyle } from "../ui/menu";
import { chooseSort, directionOf, setTrackView, sortLabel, TRACK_SORT_IDS, trackReversed, trackSort, trackView, type TrackView, viewLabel } from "./track-sort";

const VIEW_IDS: readonly TrackView[] = ["compact", "list"];

/** The way the chosen order runs (none for custom order). */
const direction = () => (trackSort() === "custom" ? null : directionOf(trackSort(), trackReversed()));
/** It as a terminal's arrow, which draws no icons. */
const arrow = () => (direction() === null ? "" : direction() === "asc" ? "↑" : "↓");
/** It in words, for screen readers. */
const spoken = () => (direction() === null ? "" : direction() === "asc" ? t("sort.asc") : t("sort.desc"));

/** The way the order runs, as an arrow icon (a terminal's text arrow). */
const Arrow = (props: { size: number; color: IconColor }) => (
  <If when={direction()}>
    {(dir) => (terminal ? <Text class="text-sm text-popover-foreground">{arrow()}</Text> : <Icon name={dir() === "asc" ? "arrowUp" : "arrowDown"} size={props.size} color={props.color} />)}
  </If>
);

const Heading = (props: { label: string }) => (
  <Text class="px-3 pb-1 pt-2 text-xs font-bold text-muted-foreground">{props.label}</Text>
);

/** A button showing the order in use, which opens a menu to change the
 * order or switch between List and Compact rows. */
export function TrackSortMenu() {
  const open = signal(false);
  const label = () => sortLabel(trackSort());
  return (
    <View class={terminal ? "flex-row justify-end px-1" : "mx-3 flex-row justify-end px-3 pb-1"}>
      <Menu
        open={open()}
        onOpenChange={(e) => open.set(e.open)}
        placement="bottom"
        align="end"
        offset={6}
        popupClass={`w-48 ${popupSurface}`}
        popupStyle={menuPopupStyle("bottom", "end")}
        popupStartingStyle={menuPopupHidden("bottom", "end")}
        popupEndingStyle={menuPopupLeaving("bottom", "end")}
        trigger={
          <Button
            accessibilityLabel={spoken() ? t("sort.labelDir", { label: label(), direction: spoken() }) : t("sort.label", { label: label() })}
            selected={open()}
            class={
              terminal
                ? "px-1 focus:bg-muted selected:bg-muted"
                : `group h-7 flex-row items-center gap-1 rounded-(--radius-inset) px-2 transition duration-200 ease-out hover:bg-row-hover selected:bg-secondary motion-reduce:transition-none ${focusRing}`
            }
          >
            <Text class="text-xs font-medium text-muted-foreground group-hover:text-foreground">{terminal ? `${label()} ${arrow()}`.trim() : label()}</Text>
            <If when={!terminal}>
              <Arrow size={12} color="muted-foreground" />
              <Icon name="chevronDown" size={12} color="muted-foreground" />
            </If>
          </Button>
        }
      >
        <Heading label={t("sort.sortBy")} />
        {TRACK_SORT_IDS.map((id) => (
          <MenuItem
            onSelect={() => chooseSort(id)}
            checked={trackSort() === id}
            accessibilityLabel={trackSort() === id && spoken() ? t("sort.itemDir", { label: sortLabel(id), direction: spoken() }) : sortLabel(id)}
            class={menuItem}
          >
            <Text class="flex-1 text-sm text-popover-foreground">{sortLabel(id)}</Text>
            <If when={trackSort() === id}>
              <Arrow size={14} color="popover-foreground" />
            </If>
            <If when={trackSort() === id && !terminal}>
              <Icon name="check" size={14} color="popover-foreground" />
            </If>
          </MenuItem>
        ))}
        <If when={!terminal}>
          <MenuSeparator class="my-1 h-px bg-border" />
          <Heading label={t("sort.viewAs")} />
          {VIEW_IDS.map((id) => (
            <MenuItem onSelect={() => setTrackView(id)} checked={trackView() === id} class={menuItem}>
              <Text class="flex-1 text-sm text-popover-foreground">{viewLabel(id)}</Text>
              <If when={trackView() === id}>
                <Icon name="check" size={14} color="popover-foreground" />
              </If>
            </MenuItem>
          ))}
        </If>
      </Menu>
    </View>
  );
}
