// The search page: a field, then songs, artists, albums and playlists.

import { resource, Scroll, token, View, windowSize } from "nsh";
import { Input } from "nsh/base";
import { store } from "../ui/store";
import { SectionTitle } from "../ui/controls";
import { Icon } from "../ui/icons";
import { CardRow, PageMessage, TrackHeader, TrackRow } from "./common";
import type { SearchResults } from "../ui/types";

export function Search() {
  // The trimmed query, and a source that also reloads on an epoch bump.
  const term = () => store.query().trim();
  const results = resource(
    () => ({ term: term(), epoch: store.epoch() }),
    async (s, { signal }) => {
      if (!s.term) return null;
      // A search per pause in typing; a newer one supersedes this one.
      await new Promise((r) => setTimeout(r, 280));
      if (signal.aborted) return null;
      return (await store.spotify.search(s.term)) as SearchResults;
    },
  );

  // Wide windows have room for the album column.
  const wide = () => windowSize().width > 1100;

  const empty = (found: SearchResults) =>
    found.tracks.length === 0 &&
    found.artists.length === 0 &&
    found.albums.length === 0 &&
    found.playlists.length === 0;

  return (
    <View class="flex-1">
      <View class="px-6 pb-4 pt-16">
        <View class="h-12 max-w-[480px] flex-row items-center gap-3 rounded-full border border-secondary bg-secondary px-4 text-muted-foreground focus-within:border-foreground">
          <Icon name="search" size={20} color="muted-foreground" />
          <Input
            value={store.query()}
            onChange={({ value }) => store.query.set(value)}
            placeholder="What do you want to play?"
            autoFocus
            class="flex-1 text-base text-foreground"
            style={{ placeholderColor: token("muted-foreground"), caretColor: token("primary") }}
          />
        </View>
      </View>
      <Scroll class="flex-1">
        <If
          when={term()}
          else={<PageMessage title="Search Spotify" detail="Find songs, artists, albums and playlists." />}
        >
          <If
            when={results.error()}
            else={
              <If
                when={results()}
                else={<PageMessage title="Searching" />}
              >
                {(found) => (
                  <If
                    when={!empty(found())}
                    else={<PageMessage title="No results" detail="Check the spelling, or try fewer words." />}
                  >
                    <View class="gap-8 pb-8">
                      <If when={found().tracks.length > 0}>
                        <View class="gap-2">
                          <SectionTitle class="px-6">Songs</SectionTitle>
                          <TrackHeader showAlbum={wide} />
                          <Each of={found().tracks} key={(t) => t.uid || t.uri}>
                            {(t, i) => (
                              <TrackRow
                                track={() => t()}
                                number={() => i()}
                                showAlbum={wide}
                                onPress={() => store.playTrack(t().uri)}
                              />
                            )}
                          </Each>
                        </View>
                      </If>
                      <If when={found().artists.length > 0}>
                        <CardRow title="Artists" items={found().artists} class="px-3" />
                      </If>
                      <If when={found().albums.length > 0}>
                        <CardRow title="Albums" items={found().albums} class="px-3" />
                      </If>
                      <If when={found().playlists.length > 0}>
                        <CardRow title="Playlists" items={found().playlists} class="px-3" />
                      </If>
                    </View>
                  </If>
                )}
              </If>
            }
          >
            <PageMessage title="Search didn't work" detail={String(results.error())} />
          </If>
        </If>
      </Scroll>
    </View>
  );
}
