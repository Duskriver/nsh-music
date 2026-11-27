// What opens over the pages for the whole app: About, the sign-in prompt
// and the first-run sheet. Each is `nsh/base`'s `Dialog` or `Sheet`, which
// own focus, Escape and back.

import { Image, keymap, Scroll, Text, View } from "nsh";
import { type NoParamTranslationKey, t } from "../i18n";
import { ActionButton, IconButton, terminal, TextLink } from "../components/ui/controls";
import { AppDialog, AppSheet } from "../components/ui/overlays";
import { NSH_URL, openExternal, REPO_URL } from "../state/links";
import { aboutOpen, finishFirstRun, firstRunOpen, setAboutOpen, setShortcutsOpen, shortcutsOpen } from "../state/overlays";
import { REGION_KEYS, type RegionName } from "../components/ui/regions";
import { cancelSignIn, health, signedIn, signIn } from "../state/session";
import { Logo } from "../ui/logo";

const REGION_LABEL_KEY: Record<RegionName, NoParamTranslationKey> = {
  sidebar: "dialogs.shortcuts.goToSidebar",
  main: "dialogs.shortcuts.goToPage",
  header: "dialogs.shortcuts.goToHeader",
  player: "dialogs.shortcuts.goToPlayer",
};

/** The keyboard, as the help lists it: what each key does, by section.
 * Several keystrokes in one row are alternatives (shown joined by "/"). */
const SHORTCUTS: { titleKey: NoParamTranslationKey; rows: { keys: string[]; labelKey: NoParamTranslationKey }[] }[] = [
  {
    titleKey: "dialogs.shortcuts.sections.regions",
    rows: REGION_KEYS.map((k) => ({ keys: [k.keystroke], labelKey: REGION_LABEL_KEY[k.region] })),
  },
  {
    titleKey: "dialogs.shortcuts.sections.inRegion",
    rows: [
      { keys: ["up", "down", "left", "right"], labelKey: "dialogs.shortcuts.moveBetweenItems" },
      { keys: ["home", "end"], labelKey: "dialogs.shortcuts.firstAndLast" },
      { keys: ["pageup", "pagedown"], labelKey: "dialogs.shortcuts.pageUpDown" },
      { keys: ["enter"], labelKey: "dialogs.shortcuts.playOrOpen" },
      { keys: ["tab", "shift+tab"], labelKey: "dialogs.shortcuts.nextPrevControl" },
    ],
  },
  {
    titleKey: "dialogs.shortcuts.sections.playback",
    rows: [
      { keys: ["space"], labelKey: "dialogs.shortcuts.playAndPause" },
      { keys: ["mod+shift+right"], labelKey: "dialogs.shortcuts.nextTrack" },
      { keys: ["mod+shift+left"], labelKey: "dialogs.shortcuts.prevTrack" },
    ],
  },
  {
    titleKey: "dialogs.shortcuts.sections.pages",
    rows: [
      { keys: ["alt+left"], labelKey: "dialogs.shortcuts.goBack" },
      { keys: ["escape"], labelKey: "dialogs.shortcuts.closeOrBack" },
      { keys: ["mod+,"], labelKey: "dialogs.shortcuts.openSettings" },
      { keys: ["shift+/"], labelKey: "dialogs.shortcuts.showShortcuts" },
    ],
  },
];

/** A keystroke as a key cap, written the platform's way. */
function KeyCap(props: { keystroke: string }) {
  return (
    <Text class={terminal ? "text-foreground" : "rounded-(--radius) border border-border bg-secondary px-1.5 py-0.5 text-xs font-medium text-foreground"}>
      {keymap.label(props.keystroke)}
    </Text>
  );
}

/** The keyboard shortcuts' help (`?`). */
export function ShortcutsDialog() {
  return (
    <AppDialog open={shortcutsOpen()} onOpenChange={setShortcutsOpen} testID="shortcuts">
      <View class="flex-row items-center justify-between pl-6 pr-3 pt-3">
        <Text role="header" class="text-lg font-extrabold text-foreground">
          {t("dialogs.shortcuts.title")}
        </Text>
        <IconButton name="close" label={t("account.close")} onPress={() => setShortcutsOpen(false)} />
      </View>
      <Scroll class="max-h-140" contentClass={terminal ? "gap-1" : "gap-5 px-6 pb-6 pt-3"}>
        <Each of={SHORTCUTS} key={(s) => s.titleKey}>
          {(section) => (
            <View class="gap-2">
              <Text class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(section().titleKey)}</Text>
              <Each of={section().rows} key={(r) => r.labelKey}>
                {(row) => (
                  <View class="min-h-7 flex-row items-center gap-4">
                    <Text class="min-w-0 flex-1 text-sm text-foreground">{t(row().labelKey)}</Text>
                    <View class="flex-row items-center gap-1">
                      <Each of={row().keys} key={(k) => k}>
                        {(k) => <KeyCap keystroke={k()} />}
                      </Each>
                    </View>
                  </View>
                )}
              </Each>
            </View>
          )}
        </Each>
      </Scroll>
    </AppDialog>
  );
}

/** Project information and links. */
export function AboutDialog() {
  return (
    <AppDialog open={aboutOpen()} onOpenChange={setAboutOpen} testID="about">
      <View class="flex-row items-center justify-end px-3 pt-3">
        <IconButton name="close" label={t("account.close")} onPress={() => setAboutOpen(false)} />
      </View>
      <Scroll class="max-h-140" contentClass={terminal ? "gap-1" : "gap-5 px-6 pb-6"}>
        <View class="flex-row items-center gap-3">
          <If when={!terminal}>
            <Logo size={44} />
          </If>
          <View selectable={!terminal}>
            <Text role="header" class="text-xl font-extrabold text-foreground">
              nsh-music
            </Text>
            <Text class="text-xs text-muted-foreground">{t("dialogs.about.subtitle")}</Text>
          </View>
        </View>
        <View class="gap-2" selectable={!terminal}>
          <Text class="text-sm font-bold text-foreground">{t("dialogs.about.disclaimer")}</Text>
          <Text class="text-sm text-muted-foreground">
            {t("dialogs.about.rights")}
          </Text>
        </View>
        <View class="flex-row flex-wrap gap-x-4 gap-y-1">
          <TextLink label={t("dialogs.about.builtWithNsh")} onPress={() => openExternal(NSH_URL)} />
          <TextLink label={t("dialogs.about.sourceOnGithub")} onPress={() => openExternal(REPO_URL)} />
        </View>
      </Scroll>
    </AppDialog>
  );
}

/** While NetEase login QR code is open. Escape, back and Cancel cancel it. */
export function SigningInDialog() {
  const open = () => health().status === "signingIn";
  const h = () => health();
  return (
    <AppDialog
      open={open()}
      onOpenChange={(next) => {
        if (!next) cancelSignIn();
      }}
      closeOnBackdropPress={false}
      testID="signing-in"
    >
      <View class={terminal ? "gap-1" : "items-center gap-4 p-8"}>
        <Text role="header" class="text-xl font-bold text-foreground">
          {t("dialogs.signingIn.title")}
        </Text>
        <If when={!terminal && !!h().qrImage}>
          <View class="items-center justify-center rounded-(--radius-card) border border-border bg-card p-2">
            <Image src={h().qrImage!} style={{ width: 180, height: 180 }} />
          </View>
        </If>
        <Text class="text-center text-sm text-muted-foreground">
          {h().prompt || t("dialogs.signingIn.prompt")}
        </Text>
        <If when={h().qrImage === ""}>
          <ActionButton label={t("dialogs.signingIn.retry")} variant="primary" onPress={signIn} />
        </If>
        <ActionButton label={t("account.cancel")} onPress={cancelSignIn} />
      </View>
    </AppDialog>
  );
}

/** Shown once, on the first launch: what nsh-music is, and how to start. */
export function FirstRunSheet() {
  return (
    <AppSheet
      open={firstRunOpen()}
      onOpenChange={(next) => {
        if (!next) finishFirstRun();
      }}
      side="bottom"
      testID="first-run"
    >
      <View class={terminal ? "gap-1" : "mx-auto w-full max-w-lg gap-4 p-6"}>
        <View class="flex-row items-center gap-3">
          <If when={!terminal}>
            <Logo size={44} />
          </If>
          <Text role="header" class="text-xl font-extrabold text-foreground">
            {t("dialogs.firstRun.welcome")}
          </Text>
        </View>
        <Text class="text-sm text-foreground">{t("dialogs.about.disclaimer")}</Text>
        <View class="flex-row flex-wrap items-center gap-3">
          <If when={signedIn() !== true}>
            <ActionButton
              label={t("account.logIn")}
              variant="primary"
              onPress={() => {
                finishFirstRun();
                signIn();
              }}
            />
          </If>
          <ActionButton label={signedIn() === true ? t("dialogs.firstRun.startListening") : t("dialogs.firstRun.notNow")} onPress={finishFirstRun} />
        </View>
        <TextLink label={t("dialogs.about.builtWithNsh")} onPress={() => openExternal(NSH_URL)} />
      </View>
    </AppSheet>
  );
}
