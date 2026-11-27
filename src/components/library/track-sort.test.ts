import { expect, test } from "nsh/test";
import type { Track } from "../../backend/types";
import { chooseSort, directionOf, sortTracks, trackReversed, trackSort } from "./track-sort";

function track(id: string, fields: Partial<Track> = {}): Track {
  return {
    uri: `netease:track:${id}`,
    uid: id,
    name: id,
    artists: "Artist",
    album: "Album",
    albumUri: `netease:album:${id}`,
    image: "",
    durationMs: 180_000,
    explicit: false,
    playable: true,
    ...fields,
  };
}

test("custom returns the tracks in their own order", () => {
  const list = [track("b"), track("a")];
  expect(sortTracks(list, "custom")).toBe(list);
  expect(sortTracks(list, "custom", true)).toBe(list);
});

test("title, artist and album sort case-insensitively with natural numbers", () => {
  const c10 = track("1", { name: "c10", artists: "B", album: "2" });
  const c2 = track("2", { name: "C2", artists: "a", album: "10" });
  const c1 = track("3", { name: "c1", artists: "C", album: "1" });
  expect(sortTracks([c10, c2, c1], "title").map((t) => t.name)).toEqual(["c1", "C2", "c10"]);
  expect(sortTracks([c10, c2, c1], "title", true).map((t) => t.name)).toEqual(["c10", "C2", "c1"]);
  expect(sortTracks([c10, c2, c1], "artist").map((t) => t.artists)).toEqual(["a", "B", "C"]);
  expect(sortTracks([c10, c2, c1], "album").map((t) => t.album)).toEqual(["1", "2", "10"]);
});

test("recently added and release date put newest first, and undated tracks last either way", () => {
  const old = track("old", { addedAt: "2024-01-01T00:00:00Z", releaseDate: "1999" });
  const empty = track("empty", { addedAt: "", releaseDate: "" });
  const mid = track("mid", { addedAt: "2025-06-01T00:00:00Z", releaseDate: "2020-05" });
  const fresh = track("fresh", { addedAt: "2026-03-01T00:00:00Z", releaseDate: "2024-11-20" });
  const list = [old, empty, mid, fresh];

  expect(sortTracks(list, "added").map((t) => t.uri)).toEqual([fresh.uri, mid.uri, old.uri, empty.uri]);
  expect(sortTracks(list, "added", true).map((t) => t.uri)).toEqual([old.uri, mid.uri, fresh.uri, empty.uri]);
  expect(sortTracks(list, "release").map((t) => t.uri)).toEqual([fresh.uri, mid.uri, old.uri, empty.uri]);
  expect(sortTracks(list, "release", true).map((t) => t.uri)).toEqual([old.uri, mid.uri, fresh.uri, empty.uri]);
});

test("duration sorts shortest first, and ties keep the playlist's order", () => {
  const long = track("long", { name: "Same", durationMs: 240_000 });
  const first = track("first", { name: "Same", durationMs: 120_000 });
  const second = track("second", { name: "Same", durationMs: 120_000 });

  expect(sortTracks([long, first, second], "duration").map((t) => t.uri)).toEqual([first.uri, second.uri, long.uri]);
  expect(sortTracks([long, first, second], "duration", true).map((t) => t.uri)).toEqual([long.uri, first.uri, second.uri]);
  expect(sortTracks([first, second], "title").map((t) => t.uri)).toEqual([first.uri, second.uri]);
});

test("choosing the sort in use again turns it round; custom keeps one way", () => {
  chooseSort("custom");
  expect(trackSort()).toBe("custom");
  expect(trackReversed()).toBe(false);

  chooseSort("title");
  expect(trackSort()).toBe("title");
  expect(directionOf(trackSort(), trackReversed())).toBe("asc");
  chooseSort("title");
  expect(directionOf(trackSort(), trackReversed())).toBe("desc");

  chooseSort("added");
  expect(trackSort()).toBe("added");
  expect(directionOf("added", trackReversed())).toBe("desc");
  chooseSort("added");
  expect(directionOf("added", trackReversed())).toBe("asc");

  chooseSort("custom");
  chooseSort("custom");
  expect(trackReversed()).toBe(false);
});
