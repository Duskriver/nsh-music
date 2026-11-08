// A collection's page: a playlist, album, artist or Liked Songs, with its
// header, a play button and its tracks in a virtualized, recycling list.
// Each opened collection is a page of its own in the stack, with its own
// list and scroll position.

import { effect, List, Text, untracked, View } from "nsh";
import type { Collection } from "../backend/types";
import { AppPage } from "../components/page";
import { layout, pageEnd, wideTracks } from "../components/layout";
import { CardGrid } from "../components/library/cards";
import { TrackHeader, TrackRow } from "../components/library/tracks";
import {
  Cover,
  PlayButton,
  SectionTitle,
  terminal,
} from "../components/ui/controls";
import { AFTER_REVEAL, createListWave, enter } from "../components/ui/motion";
import { currentRoute } from "../nav";
import {
  EmptyState,
  ErrorState,
  CollectionSkeleton,
  Reveal,
} from "../components/ui/feedback";
import { collectionQuery } from "../data/queries";
import { isPlayingContext, playFrom, rememberTracks, toggleCollection } from "../state/player";
import { ready } from "../state/session";

const KIND_LABEL: Record<string, string> = {
  album: "Album",
  artist: "Artist",
  collection: "Playlist",
  playlist: "Playlist",
};

function Header(props: { uri: string; data: Collection; entrance: boolean }) {
  // Its layers come in one after another: at once when the page shows
  // straight away (cached), else as the skeleton gives way. Made once.
  const start = untracked(() => (props.entrance ? 0 : AFTER_REVEAL));
  const intro = (preset: "zoomIn" | "fadeInUp", delay = 0) =>
    untracked(() => (terminal ? undefined : enter(preset, start + delay)));
  const coverIn = intro("zoomIn");
  // The words, each line on its own beat: the kind, then the name, then the
  // quieter lines under it.
  const kindIn = intro("fadeInUp", 70);
  const nameIn = intro("fadeInUp", 150);
  const restIn = intro("fadeInUp", 250);
  const playIn = intro("zoomIn", 330);
  const header = () => props.data.header;
  const phone = () => layout() !== "desktop";
  const play = (size: number) => (
    <PlayButton
      playing={isPlayingContext(props.uri)}
      size={size}
      disabled={!ready() || props.data.tracks.length === 0}
      onPress={() => toggleCollection(props.uri)}
    />
  );
  return (
    <View
      class={
        terminal
          ? "gap-1"
          : phone()
            ? "items-center gap-3 px-6 pb-4 pt-1"
            : // Desktop: the header's picture (96pt) is centred over the rows'
              // covers: a row puts its cover's centre 78pt in (12 + 12 inset,
              // 24 number, 12 gap, half of 36), so the header starts 30pt in.
              "flex-row items-end gap-4 pl-[30px] pr-6 pb-4 pt-1"
      }
    >
      <View style={coverIn} class={phone() ? "w-full max-w-40" : ""}>
        <Cover
          src={header().image}
          round={header().kind === "artist"}
          liked={header().kind === "collection"}
          class={phone() ? "w-full shadow-xl" : "w-24 shadow-xl"}
        />
      </View>
      {/* The words as one block centred on the picture: kind, name, and a
          quiet third line (secondary: small and muted). */}
      <View class="min-w-0 flex-1 gap-1 self-stretch justify-center" selectable={!terminal}>
        <View style={kindIn}>
          <Text class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            {KIND_LABEL[header().kind] ?? "Playlist"}
          </Text>
        </View>
        <View style={nameIn}>
          <Text role="header" class={`line-clamp-1 font-extrabold text-foreground ${phone() ? "text-2xl" : "text-3xl"}`}>
            {header().name}
          </Text>
        </View>
        <View class="gap-1" style={restIn}>
          <If when={header().description}>
            <Text class="line-clamp-1 text-sm text-muted-foreground">{header().description}</Text>
          </If>
          <Text class="text-xs text-muted-foreground">{header().subtitle}</Text>
        </View>
      </View>
      <View class="shrink-0" style={playIn}>
        {play(phone() ? 48 : 44)}
      </View>
    </View>
  );
}

function Tracks(props: { uri: string; data: Collection; instant: boolean }) {
  // What plays is found among these when the player can't name it.
  effect(() => rememberTracks(props.uri, props.data.tracks));
  const artist = () => props.data.header.kind === "artist";
  const showAlbum = () => wideTracks() && !artist();
  // The rows wave in, top to bottom, every time this page is entered
  // (opened, or gone back to): at once when cached, as the skeleton fades
  // when they loaded. Rows scrolled to come in on their own.
  const wave = untracked(() =>
    createListWave({
      isTop: () => {
        const route = currentRoute();
        return route.name === "collection" && route.uri === props.uri;
      },
      afterReveal: !props.instant,
    }),
  );
  return (
    <List
      each={props.data.tracks}
      keyExtractor={(t) => t.uid + t.uri}
      estimatedItemSize={48}
      header={() => (
        <View>
          <Header uri={props.uri} data={props.data} entrance={props.instant} />
          <TrackHeader showAlbum={showAlbum()} />
        </View>
      )}
      footer={() => (
        <View class={pageEnd()}>
          <If when={props.data.releases.length > 0}>
            <View class="gap-2 px-3 pt-8">
              <SectionTitle class="px-3">Discography</SectionTitle>
              <CardGrid items={props.data.releases} wave={wave} />
            </View>
          </If>
        </View>
      )}
      empty={() => <EmptyState title="Nothing here yet" />}
      class="flex-1"
    >
      {(row, index) => (
        <TrackRow
          track={row()}
          number={index()}
          showAlbum={showAlbum()}
          secondary={artist() ? "album" : "artists"}
          disabled={!ready()}
          wave={wave}
          onPress={() => {
            const track = row();
            if (track) playFrom(props.uri, track.uri, track);
          }}
        />
      )}
    </List>
  );
}

export function CollectionPage(props: { uri: string }) {
  const query = collectionQuery(() => props.uri);
  const title = () => query.data()?.header.name ?? "";
  const liked = () => query.data()?.header.kind === "collection";
  const body = () => (
    <Reveal
      data={query.data()}
      placeholder={
        <If when={query.error()} else={<CollectionSkeleton phone={layout() !== "desktop"} />}>
          <ErrorState
            message="This page didn't load."
            onRetry={query.refetch}
          />
        </If>
      }
      class="flex-1"
    >
      {(data, instant) => <Tracks uri={props.uri} data={data()} instant={instant} />}
    </Reveal>
  );
  return (
    <AppPage
      title={title()}
      testID={`page-${props.uri}`}
    >
      {body()}
    </AppPage>
  );
}
