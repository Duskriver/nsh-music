// Settings: playback quality & mode, language, appearance, music light,
// desktop tray behavior, local cache, account and updates.

import { fpsEnabled, Scroll, setFpsEnabled, signal, Text, untracked, View } from "nsh";
import { Button, Menu, MenuItem, MenuSeparator } from "nsh/base";
import { t } from "../i18n";
import { Version } from "nsh-music/native";
import type { AudioQuality, PlayMode } from "../backend/types";
import { layout, pageEnd } from "../components/layout";
import type { TrackView } from "../components/library/track-sort";
import { setTrackView, trackView } from "../components/library/track-sort";
import { AppPage } from "../components/page";
import {
  ActionButton,
  Cover,
  focusRing,
  menuItem,
  popupSurface,
  SegmentedControl,
  SettingGroup,
  SettingRow,
  SettingSwitch,
  terminal,
} from "../components/ui/controls";
import { Icon } from "../components/ui/icons";
import { menuPopupHidden, menuPopupLeaving, menuPopupStyle } from "../components/ui/menu";
import { enter } from "../components/ui/motion";
import { confirmSignOut } from "../components/ui/overlays";
import { TOOLBAR_EFFECTS } from "../components/ui/toolbar-fx";
import { cacheSize, clearAccount } from "../data/cache";
import { meQuery } from "../data/queries";
import { currentRoute } from "../nav";
import { hasAppWindow } from "../platform/desktop";
import { usingSystemTheme } from "../platform/theme";
import { checkForUpdates } from "../platform/updater";
import { isDemo } from "../state/engine";
import { announce } from "../state/feedback";
import { lyricsShown, setAboutOpen, setLyricsShown, setShortcutsOpen } from "../state/overlays";
import { player, setPlayMode, setQuality } from "../state/player";
import { cacheAccount, ready, signedIn, signIn } from "../state/session";
import {
  appearancePreference,
  type AppearancePreference,
  hideOnClose,
  languagePreference,
  type LanguagePreference,
  setAppearancePreference,
  setHideOnClose,
  setLanguagePreference,
} from "../state/settings";
import { effectLabel, lightEffect, lightName, lightRandom, setLight } from "../state/visual";

const qualityOptions = (): { value: AudioQuality; label: string; detail: string }[] => [
  { value: "standard", label: t("player.quality.standard"), detail: "128k" },
  { value: "exhigh", label: t("player.quality.exhigh"), detail: "320k" },
  { value: "lossless", label: t("player.quality.lossless"), detail: "FLAC" },
  { value: "hires", label: t("player.quality.hires"), detail: "FLAC" },
];

const playModeOptions = (): { value: PlayMode; label: string }[] => [
  { value: "repeatAll", label: t("player.playMode.repeatAll") },
  { value: "repeatOne", label: t("player.playMode.repeatOne") },
  { value: "shuffle", label: t("player.playMode.shuffle") },
];

const languageOptions = (): { value: LanguagePreference; label: string }[] => [
  { value: "system", label: t("settings.language.system") },
  { value: "zh-CN", label: t("settings.language.zhCN") },
  { value: "en", label: t("settings.language.en") },
];

const appearanceOptions = (): { value: AppearancePreference; label: string }[] => [
  { value: "system", label: t("settings.theme.system") },
  { value: "light", label: t("settings.theme.light") },
  { value: "dark", label: t("settings.theme.dark") },
];

const densityOptions = (): { value: TrackView; label: string }[] => [
  { value: "list", label: t("sort.views.list") },
  { value: "compact", label: t("sort.views.compact") },
];

function appVersion(): string {
  try {
    return Version.crateVersion() || "0.1.6";
  } catch {
    return "0.1.6";
  }
}

function MusicLightSelect() {
  const open = signal(false);
  return (
    <Menu
      open={open()}
      onOpenChange={(e) => open.set(e.open)}
      placement="bottom"
      align="end"
      offset={6}
      popupClass={`w-52 ${popupSurface}`}
      popupStyle={menuPopupStyle("bottom", "end")}
      popupStartingStyle={menuPopupHidden("bottom", "end")}
      popupEndingStyle={menuPopupLeaving("bottom", "end")}
      trigger={
        <Button
          accessibilityLabel={t("settings.musicLight.a11y", { name: lightName() })}
          selected={open()}
          class={`h-9 flex-row items-center gap-2 rounded-full border border-border bg-secondary px-4 transition duration-180 ease-out hover:border-muted-foreground hover:bg-row-hover active:scale-[0.97] selected:border-muted-foreground selected:bg-muted motion-reduce:transition-none ${focusRing}`}
        >
          <Text class="text-sm font-semibold text-foreground">{lightName()}</Text>
          <Icon name="chevronDown" size={14} color="muted-foreground" />
        </Button>
      }
    >
      <MenuItem
        role="menuitemradio"
        checked={lightRandom()}
        onSelect={() => setLight("random")}
        class={`group ${menuItem} gap-2.5`}
        indicatorClass="hidden"
      >
        <View class="w-[15px] items-start">
          <Icon name="shuffle" size={12} color="muted-foreground" />
        </View>
        <Text class="line-clamp-1 flex-1 text-sm text-popover-foreground group-checked:font-semibold">{t("player.light.random")}</Text>
        <If when={lightRandom()}>
          <Icon name="check" size={12} color="foreground" />
        </If>
      </MenuItem>
      <MenuSeparator class="-mx-1 my-1 h-px bg-border" />
      <Scroll class="max-h-72">
        <Each of={TOOLBAR_EFFECTS} key={(e) => e}>
          {(effect) => (
            <MenuItem
              role="menuitemradio"
              checked={!lightRandom() && lightEffect() === effect()}
              onSelect={() => setLight(effect())}
              class={`group ${menuItem} gap-2.5`}
              indicatorClass="hidden"
            >
              <Text class="font-mono text-xs tabular-nums text-muted-foreground">
                {String(TOOLBAR_EFFECTS.indexOf(effect()) + 1).padStart(2, "0")}
              </Text>
              <Text class="line-clamp-1 flex-1 text-sm text-popover-foreground group-checked:font-semibold">
                {effectLabel(effect())}
              </Text>
              <If when={!lightRandom() && lightEffect() === effect()}>
                <Icon name="check" size={12} color="foreground" />
              </If>
              <If when={lightRandom() && lightEffect() === effect()}>
                <Icon name="shuffle" size={12} color="muted-foreground" />
              </If>
            </MenuItem>
          )}
        </Each>
      </Scroll>
    </Menu>
  );
}

export function SettingsPage() {
  const me = meQuery();
  const clearing = signal(false);
  const checkingUpdate = signal(false);
  const compactLayout = () => layout() !== "desktop" || terminal;
  const groupIn = (index: number) => untracked(() => (terminal ? undefined : enter("fadeInUp", index * 55)));

  const titleIn = groupIn(0);
  const playbackIn = groupIn(1);
  const appearanceIn = groupIn(2);
  const behaviorIn = groupIn(3);
  const storageIn = groupIn(4);
  const accountIn = groupIn(5);

  const clearCache = async () => {
    if (clearing()) return;
    clearing.set(true);
    try {
      const account = cacheAccount();
      if (account) await clearAccount(account);
      announce(t("settings.cache.cleared"));
    } finally {
      clearing.set(false);
    }
  };

  const runUpdateCheck = async () => {
    if (checkingUpdate() || isDemo) return;
    checkingUpdate.set(true);
    try {
      await checkForUpdates({
        notify: (message, action, secondary) => announce(message, action, secondary),
        onUpToDate: () => announce(t("settings.updates.upToDate")),
      });
    } finally {
      checkingUpdate.set(false);
    }
  };

  const cacheCount = () => cacheSize();
  const cacheDescription = () => {
    const count = cacheCount();
    if (count === 0) return t("settings.cache.empty");
    if (count === 1) return t("settings.cache.one");
    return t("settings.cache.other", { count });
  };

  return (
    <AppPage title={t("settings.title")} testID="page-settings" shown={() => currentRoute().name === "settings"}>
      <Scroll
        class="flex-1"
        contentClass={terminal ? "gap-2 px-1 pb-4" : `mx-auto w-full max-w-2xl gap-6 px-4 pt-2 ${pageEnd()}`}
      >
        <Text
          role="header"
          class={terminal ? "font-bold text-foreground" : "px-1 text-3xl font-extrabold text-foreground"}
          style={titleIn}
        >
          {t("settings.title")}
        </Text>

        <SettingGroup title={t("settings.sections.playback")} style={playbackIn}>
          <SettingRow
            first
            stacked
            title={t("settings.audioQuality.title")}
            description={t("settings.audioQuality.description")}
          >
            <SegmentedControl
              label={t("settings.audioQuality.title")}
              value={player().quality}
              options={qualityOptions()}
              disabled={!ready()}
              onChange={setQuality}
            />
          </SettingRow>
          <SettingRow
            stacked={compactLayout()}
            title={t("settings.playMode.title")}
            description={t("settings.playMode.description")}
          >
            <SegmentedControl
              label={t("settings.playMode.title")}
              value={player().playMode}
              options={playModeOptions()}
              disabled={!ready()}
              onChange={setPlayMode}
            />
          </SettingRow>
          <SettingRow
            title={t("settings.lyricsDefault.title")}
            description={t("settings.lyricsDefault.description")}
          >
            <SettingSwitch
              label={t("settings.lyricsDefault.title")}
              checked={lyricsShown()}
              onChange={setLyricsShown}
            />
          </SettingRow>
        </SettingGroup>

        <SettingGroup title={t("settings.sections.appearance")} style={appearanceIn}>
          <SettingRow
            first
            stacked={compactLayout()}
            title={t("settings.language.title")}
            description={t("settings.language.description")}
          >
            <SegmentedControl
              label={t("settings.language.title")}
              value={languagePreference()}
              options={languageOptions()}
              onChange={setLanguagePreference}
            />
          </SettingRow>
          <SettingRow
            stacked={compactLayout()}
            title={t("settings.theme.title")}
            description={
              usingSystemTheme()
                ? t("settings.theme.controlledBySystem")
                : t("settings.theme.description")
            }
          >
            <SegmentedControl
              label={t("settings.theme.title")}
              value={appearancePreference()}
              options={appearanceOptions()}
              disabled={usingSystemTheme()}
              onChange={setAppearancePreference}
            />
          </SettingRow>
          <If when={!terminal}>
            <SettingRow
              title={t("settings.musicLight.title")}
              description={t("settings.musicLight.description")}
            >
              <MusicLightSelect />
            </SettingRow>
            <SettingRow
              title={t("settings.density.title")}
              description={t("settings.density.description")}
            >
              <SegmentedControl
                label={t("settings.density.title")}
                value={trackView()}
                options={densityOptions()}
                onChange={setTrackView}
              />
            </SettingRow>
          </If>
        </SettingGroup>

        <If when={hasAppWindow || !terminal}>
          <SettingGroup title={t("settings.sections.behavior")} style={behaviorIn}>
            <If when={hasAppWindow}>
              <SettingRow
                first
                title={t("settings.hideOnClose.title")}
                description={t("settings.hideOnClose.description")}
              >
                <SettingSwitch
                  label={t("settings.hideOnClose.title")}
                  checked={hideOnClose()}
                  onChange={setHideOnClose}
                />
              </SettingRow>
            </If>
            <If when={!terminal}>
              <SettingRow
                first={!hasAppWindow}
                title={t("settings.fps.title")}
                description={t("settings.fps.description")}
              >
                <SettingSwitch
                  label={t("settings.fps.title")}
                  checked={fpsEnabled()}
                  onChange={setFpsEnabled}
                />
              </SettingRow>
            </If>
          </SettingGroup>
        </If>

        <SettingGroup title={t("settings.sections.storage")} style={storageIn}>
          <SettingRow
            first
            title={t("settings.cache.title")}
            description={cacheDescription()}
          >
            <ActionButton
              label={t("settings.cache.clear")}
              disabled={cacheCount() === 0 || clearing()}
              onPress={() => void clearCache()}
            />
          </SettingRow>
        </SettingGroup>

        <SettingGroup title={t("settings.sections.account")} style={accountIn}>
          <SettingRow
            first
            title={signedIn() === true ? (me.data()?.name || t("account.account")) : t("settings.account.notSignedIn")}
            description={signedIn() === true ? t("settings.account.signedInDesc") : t("settings.account.signedOutDesc")}
          >
            <If
              when={signedIn() === true}
              else={<ActionButton label={t("account.logIn")} variant="primary" onPress={signIn} />}
            >
              <View class="flex-row items-center gap-3">
                <If when={!terminal}>
                  <Cover src={me.data()?.image ?? ""} size={32} round fallback="user" />
                </If>
                <ActionButton label={t("account.logOut")} onPress={() => void confirmSignOut()} />
              </View>
            </If>
          </SettingRow>
          <SettingRow
            title="nsh-music"
            description={t("settings.updates.version", { version: appVersion() })}
          >
            <If when={!isDemo}>
              <ActionButton
                label={checkingUpdate() ? t("settings.updates.checking") : t("settings.updates.check")}
                disabled={checkingUpdate()}
                onPress={() => void runUpdateCheck()}
              />
            </If>
          </SettingRow>
          <SettingRow
            stacked={compactLayout()}
            title={t("settings.help.title")}
            description={t("settings.help.description")}
          >
            <View class="flex-row flex-wrap gap-2">
              <ActionButton label={t("settings.help.shortcuts")} onPress={() => setShortcutsOpen(true)} />
              <ActionButton label={t("account.aboutEllipsis")} onPress={() => setAboutOpen(true)} />
            </View>
          </SettingRow>
        </SettingGroup>
      </Scroll>
    </AppPage>
  );
}
