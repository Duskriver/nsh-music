// Home: the greeting, then the sections of the day.

import { List, Text, untracked, View } from "nsh";
import { pageEnd } from "../components/layout";
import { homeQuery } from "../data/queries";
import { CardRow } from "../components/library/cards";
import { CardsSkeleton, ErrorState, Reveal } from "../components/ui/feedback";
import { terminal } from "../components/ui/controls";
import { createListWave, enter, waveDelay } from "../components/ui/motion";
import { currentRoute } from "../nav";
import { AppPage } from "../components/page";

/** The greeting. It rises in only when made with `entrance` (the page
 * opening): made again as the list's header, it just shows. */
function Greeting(props: { text: string; entrance?: boolean }) {
  const style = untracked(() => (props.entrance && !terminal ? enter("fadeInUp") : undefined));
  return (
    <Text role="header" class="px-3 text-4xl font-extrabold text-foreground" style={style}>
      {props.text}
    </Text>
  );
}

export function HomePage() {
  const home = homeQuery();
  return (
    <AppPage title="Home" testID="page-home">
      {/* Virtualized both ways: the sections down the page are a list, and
          each section's cards a list across, so only what is near the view
          exists. */}
      <Reveal
        data={home.data()}
        placeholder={
          <View class={terminal ? "gap-1" : `gap-8 px-3 ${pageEnd()}`}>
            <Greeting text="Home" entrance />
            <If when={home.error()} else={<CardsSkeleton />}>
              <ErrorState message="Home didn't load." onRetry={home.refetch} />
            </If>
          </View>
        }
        class="flex-1"
      >
        {(data, instant) => {
          // Every time Home is entered, each section's cards come in as a
          // wave, each section a little after the one above; cards scrolled
          // to come in on their own.
          const wave = createListWave({ isTop: () => currentRoute().name === "home", afterReveal: !instant });
          return (
          <List
            each={data().sections}
            keyExtractor={(s) => s.title}
            estimatedItemSize={terminal ? 4 : 270}
            class="flex-1"
            contentClass={terminal ? "" : "px-3"}
            header={() => <Greeting text={data().greeting || "Home"} entrance={instant} />}
            footer={() => <View class={pageEnd()} />}
          >
            {(section, i) => (
              <If when={section()}>
                {(s) => (
                  <CardRow
                    title={s().title}
                    items={s().items}
                    wave={wave}
                    delay={waveDelay(i(), 4)}
                    class={terminal ? "" : "pt-8"}
                  />
                )}
              </If>
            )}
          </List>
          );
        }}
      </Reveal>
    </AppPage>
  );
}
