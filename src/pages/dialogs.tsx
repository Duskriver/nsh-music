// What opens over the pages for the whole app: About, the sign-in prompt
// and the first-run sheet. Each is `nsh/base`'s `Dialog` or `Sheet`, which
// own focus, Escape and back.

import { Image, Scroll, Text, View } from "nsh";
import { ActionButton, IconButton, terminal, TextLink } from "../components/ui/controls";
import { AppDialog, AppSheet } from "../components/ui/overlays";
import { NSH_URL, openExternal, REPO_URL } from "../state/links";
import { aboutOpen, finishFirstRun, firstRunOpen, setAboutOpen } from "../state/overlays";
import { cancelSignIn, health, signedIn, signIn } from "../state/session";
import { Logo } from "../ui/logo";

const DISCLAIMER = "nsh-music is an unofficial client for NetEase Cloud Music.";

/** Project information and links. */
export function AboutDialog() {
  return (
    <AppDialog open={aboutOpen()} onOpenChange={setAboutOpen} testID="about">
      <View class="flex-row items-center justify-end px-3 pt-3">
        <IconButton name="close" label="Close" onPress={() => setAboutOpen(false)} />
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
            <Text class="text-xs text-muted-foreground">A native NetEase Cloud Music player</Text>
          </View>
        </View>
        <View class="gap-2" selectable={!terminal}>
          <Text class="text-sm font-bold text-foreground">{DISCLAIMER}</Text>
          <Text class="text-sm text-muted-foreground">
            Music, lyrics and artwork belong to their respective rights holders.
          </Text>
        </View>
        <View class="flex-row flex-wrap gap-x-4 gap-y-1">
          <TextLink label="Built with NSH" onPress={() => openExternal(NSH_URL)} />
          <TextLink label="Source on GitHub" onPress={() => openExternal(REPO_URL)} />
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
          扫码登录网易云音乐
        </Text>
        <If when={!terminal && !!h().qrImage}>
          <View class="items-center justify-center rounded-(--radius-card) border border-border bg-card p-2">
            <Image src={h().qrImage!} style={{ width: 180, height: 180 }} />
          </View>
        </If>
        <Text class="text-center text-sm text-muted-foreground">
          {h().prompt || "请使用网易云音乐手机 App 扫码并在手机上确认登录"}
        </Text>
        <If when={h().qrImage === ""}>
          <ActionButton label="重新扫码" variant="primary" onPress={signIn} />
        </If>
        <ActionButton label="Cancel" onPress={cancelSignIn} />
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
            Welcome to nsh-music
          </Text>
        </View>
        <Text class="text-sm text-foreground">{DISCLAIMER}</Text>
        <View class="flex-row flex-wrap items-center gap-3">
          <If when={signedIn() !== true}>
            <ActionButton
              label="Log in"
              variant="primary"
              onPress={() => {
                finishFirstRun();
                signIn();
              }}
            />
          </If>
          <ActionButton label={signedIn() === true ? "Start listening" : "Not now"} onPress={finishFirstRun} />
        </View>
        <TextLink label="Built with NSH" onPress={() => openExternal(NSH_URL)} />
      </View>
    </AppSheet>
  );
}
