// A collection's page: a playlist, album, artist or Liked Songs, with its
// header, a play button and its tracks in a virtualized, recycling list.

import { List, resource, Text, View, windowSize } from "nsh";
import { store } from "../ui/store";
import { Cover, PlayButton, SectionTitle } from "../ui/controls";
import { CardGrid, PageMessage, TrackHeader, TrackRow } from "./common";
import type { Collection as CollectionData } from "../ui/types";

function kindOf(uri: string): string {
  return uri.split(":")[1] ?? "";
}

/** What plays a collection, to tell whether it is the one playing. */
function hrefOf(uri: string): string {
  const parts = uri.split(":");
  const kind = parts[1] ?? "";
  const id = parts[2] ?? "";
  if (kind === "collection") return "/collection/tracks";
  if (kind && id) return `/${kind}/${id}`;
  return "";
}

export function Collection(props: { uri: string }) {
  const data = resource(
    () => ({ uri: props.uri, epoch: store.epoch() }),
    (src) => {
      const kind = kindOf(src.uri);
      if (kind === "album") return store.spotify.album(src.uri) as Promise<CollectionData>;
      if (kind === "artist") return store.spotify.artist(src.uri) as Promise<CollectionData>;
      if (kind === "collection") return store.spotify.likedSongs() as Promise<CollectionData>;
      return store.spotify.playlist(src.uri) as Promise<CollectionData>;
    },
  );
  return (
    <View class="flex-1">
      <If
        when={data()}
        else={
          <If when={data.error()} else={<PageMessage title="Loading" />}>
            <PageMessage title="This page didn't load" detail={String(data.error())} />
          </If>
        }
      >
        {(c) => <Page uri={props.uri} data={c} />}
      </If>
    </View>
  );
}

function Header(props: { uri: string; data: () => CollectionData }) {
  const header = () => props.data().header;
  const artist = () => header().kind === "artist";
  const liked = () => header().kind === "collection";
  const label = () => {
    const kind = header().kind;
    if (kind === "album") return "Album";
    if (kind === "artist") return "Artist";
    return "Playlist";
  };
  const playingHere = () => {
    const href = hrefOf(props.uri);
    const p = store.player();
    return p.playing && href !== "" && p.contextHref.endsWith(href);
  };
  const onPlay = () => {
    if (playingHere()) {
      store.toggle();
      return;
    }
    if (artist()) {
      // The artist page's tracks have no context of their own here.
      const first = props.data().tracks[0];
      if (first) store.playTrack(first.uri);
    } else {
      store.playItem({ uri: props.uri, kind: "playlist" });
    }
  };
  return (
    <View class="gap-6">
      <View class="flex-row items-end gap-6 px-6 pt-16">
        <Cover
          src={() => header().image}
          size={200}
          round={artist()}
          fallback={liked() ? "heartFilled" : "music"}
          class="shadow-xl"
        />
        <View class="min-w-0 flex-1 gap-2">
          <Text class="text-sm font-medium text-foreground">{label()}</Text>
          <Text class="line-clamp-2 text-[56px] leading-[62px] font-extrabold text-foreground">
            {header().name}
          </Text>
          <If when={header().description}>
            <Text class="line-clamp-2 text-sm text-muted-foreground">{header().description}</Text>
          </If>
          <Text class="text-sm font-semibold text-foreground">{header().subtitle}</Text>
        </View>
      </View>
      <View class="flex-row items-center px-6 pb-2">
        <PlayButton
          playing={playingHere}
          size={56}
          primary
          disabled={() => props.data().tracks.length === 0}
          onPress={onPlay}
        />
      </View>
    </View>
  );
}

function Page(props: { uri: string; data: () => CollectionData }) {
  const artist = () => props.data().header.kind === "artist";
  const showAlbum = () => windowSize().width > 1100 && !artist();
  const header = () => (
    <View>
      <Header uri={props.uri} data={props.data} />
      <If when={artist()}>
        <SectionTitle class="px-6 pb-2">Popular</SectionTitle>
      </If>
      <TrackHeader showAlbum={showAlbum} />
    </View>
  );
  const footer = () => (
    <View class="pb-8">
      <If when={props.data().releases.length > 0}>
        <View class="gap-2 px-3 pt-8">
          <SectionTitle class="px-3">Discography</SectionTitle>
          <CardGrid items={props.data().releases} />
        </View>
      </If>
    </View>
  );
  return (
    <List
      each={props.data().tracks}
      keyExtractor={(t) => t.uid + t.uri}
      estimatedItemSize={56}
      header={header}
      footer={footer}
      empty={() => <PageMessage title="Nothing here yet" class="pt-0" />}
      class="flex-1"
    >
      {(row, index) => (
        <TrackRow
          track={row}
          number={index}
          showAlbum={showAlbum}
          onPress={() => {
            const track = row();
            if (!track) return;
            if (artist()) store.playTrack(track.uri);
            else store.playAt(props.uri, index());
          }}
        />
      )}
    </List>
  );
}
