// The home page: Spotify's sections of the day, as rows of cards.

import { resource, Scroll, Text, View } from "nsh";
import { store } from "../ui/store";
import { CardRow, PageMessage } from "./common";
import type { Home as HomeData } from "../ui/types";

export function Home() {
  const home = resource(store.epoch, () => store.spotify.home() as Promise<HomeData>);
  return (
    <Scroll class="flex-1">
      <View class="gap-8 px-3 pb-8 pt-16">
        <Text class="px-3 text-3xl font-bold text-foreground">
          {home()?.greeting || "Home"}
        </Text>
        <If
          when={home()}
          else={
            <If
              when={home.error()}
              else={<PageMessage title="Loading" detail="Fetching what's new for you…" />}
            >
              <PageMessage title="Home didn't load" detail={String(home.error())} />
            </If>
          }
        >
          {(data) => (
            <View class="gap-8">
              <Each of={data().sections} key={(s) => s.title}>
                {(section) => <CardRow title={section().title} items={section().items} />}
              </Each>
            </View>
          )}
        </If>
      </View>
    </Scroll>
  );
}
