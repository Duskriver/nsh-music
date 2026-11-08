// Spotish's bridge, installed in every document of the Spotify web player
// (CDP `Page.addScriptToEvaluateOnNewDocument`) before the player's own
// scripts run.
//
// It learns what the player itself uses to talk to Spotify (the headers of its
// GraphQL requests, the persisted queries' hashes in its bundles), so the app
// can ask for the same data, and it reports the player's state to the app and
// presses the player's controls for it. Nothing here is a published API: every
// selector and field name the page offers is read in one place below, so a
// change of the web player is mended here.
(() => {
  if (window.__spotish) return;

  const post = (message) => {
    try {
      if (typeof window.spotishEvent === "function") window.spotishEvent(JSON.stringify(message));
    } catch (_) {}
  };

  const state = {
    headers: null, // the last GraphQL request's headers
    ops: {}, // operationName -> sha256 of its persisted query
    vars: {}, // operationName -> the variables the player last sent with it
    opsLoaded: null, // the promise of the bundles' scan
    last: "", // the last player state posted, as JSON
  };

  // --- Learning the player's requests -------------------------------------

  const KEEP = ["authorization", "client-token", "spotify-app-version", "app-platform", "accept-language"];
  const headerObject = (headers) => {
    const out = {};
    if (!headers) return out;
    if (headers instanceof Headers) headers.forEach((v, k) => (out[k.toLowerCase()] = v));
    else if (Array.isArray(headers)) headers.forEach(([k, v]) => (out[k.toLowerCase()] = v));
    else Object.keys(headers).forEach((k) => (out[k.toLowerCase()] = headers[k]));
    return out;
  };
  const learn = (url, init, request) => {
    try {
      if (!String(url).includes("/pathfinder/")) return;
      const headers = Object.assign(headerObject(request && request.headers), headerObject(init && init.headers));
      if (!headers.authorization) return;
      const kept = {};
      KEEP.forEach((k) => headers[k] && (kept[k] = headers[k]));
      const changed = !state.headers || state.headers.authorization !== kept.authorization;
      state.headers = kept;
      if (changed) post({ type: "auth" });
      const body = init && typeof init.body === "string" ? JSON.parse(init.body) : null;
      const hash = body && body.extensions && body.extensions.persistedQuery && body.extensions.persistedQuery.sha256Hash;
      if (body && body.operationName && hash) {
        state.ops[body.operationName] = hash;
        if (body.variables) state.vars[body.operationName] = body.variables;
      }
    } catch (_) {}
  };
  const originalFetch = window.fetch;
  window.fetch = function (input, init) {
    const request = input instanceof Request ? input : null;
    learn(request ? request.url : input, init, request);
    return originalFetch.apply(this, arguments);
  };

  const scanBundles = () => {
    if (state.opsLoaded) return state.opsLoaded;
    state.opsLoaded = (async () => {
      const urls = new Set();
      performance.getEntriesByType("resource").forEach((e) => /\.js(\?|$)/.test(e.name) && urls.add(e.name));
      document.querySelectorAll("script[src]").forEach((s) => urls.add(s.src));
      await Promise.all(
        [...urls].map(async (url) => {
          try {
            const text = await (await originalFetch(url)).text();
            for (const m of text.matchAll(/"([A-Za-z0-9_]+)","(?:query|mutation)","([0-9a-f]{64})"/g)) {
              if (!state.ops[m[1]]) state.ops[m[1]] = m[2];
            }
          } catch (_) {}
        }),
      );
    })();
    return state.opsLoaded;
  };

  const waitFor = async (test, timeout = 15000, every = 100) => {
    const end = Date.now() + timeout;
    for (;;) {
      const value = test();
      if (value) return value;
      if (Date.now() > end) return null;
      await new Promise((r) => setTimeout(r, every));
    }
  };

  // A GraphQL query of the player's, with its headers.
  const query = async (operationName, variables) => {
    if (!state.headers) await waitFor(() => state.headers, 20000);
    if (!state.headers) throw new Error("the web player has not signed its requests yet");
    if (!state.ops[operationName]) await scanBundles();
    if (!state.ops[operationName]) {
      state.opsLoaded = null; // bundles load lazily: look again
      await scanBundles();
    }
    const hash = state.ops[operationName];
    if (!hash) throw new Error("unknown query " + operationName);
    const response = await originalFetch("https://api-partner.spotify.com/pathfinder/v2/query", {
      method: "POST",
      headers: Object.assign({ "content-type": "application/json;charset=UTF-8", accept: "application/json" }, state.headers),
      body: JSON.stringify({
        // What the player sends, with what the app asks for over it: a query
        // that gained a variable keeps working.
        variables: Object.assign({}, state.vars[operationName] || {}, variables),
        operationName,
        extensions: { persistedQuery: { version: 1, sha256Hash: hash } },
      }),
    });
    if (!response.ok) throw new Error(operationName + ": HTTP " + response.status);
    const json = await response.json();
    if (json.errors && !json.data) throw new Error(operationName + ": " + json.errors.map((e) => e.message).join("; "));
    return json.data;
  };

  // --- Normalizing what the queries answer --------------------------------
  // Into the few shapes the app shows: items (playlists, albums, artists,
  // shows) and tracks.

  const pickImage = (sources, prefer = 300) => {
    if (!sources || !sources.length) return "";
    const sorted = sources.filter((s) => s && s.url).slice().sort((a, b) => (a.width || 0) - (b.width || 0));
    return (sorted.find((s) => (s.width || 0) >= prefer) || sorted[sorted.length - 1] || {}).url || "";
  };
  const imageOf = (data) => {
    if (!data) return "";
    const c = data.coverArt || data.images || data.visuals || data.avatar || data.albumOfTrack?.coverArt;
    if (c?.sources) return pickImage(c.sources);
    if (c?.items?.[0]?.sources) return pickImage(c.items[0].sources);
    if (c?.avatarImage?.sources) return pickImage(c.avatarImage.sources);
    if (data.visuals?.avatarImage?.sources) return pickImage(data.visuals.avatarImage.sources);
    if (data.image?.sources) return pickImage(data.image.sources);
    return "";
  };
  const artistNames = (artists) => (artists?.items || []).map((a) => a.profile?.name || a.name).filter(Boolean).join(", ");
  const kindOf = (uri) => (uri || "").split(":")[1] || "";

  const item = (data) => {
    if (!data || !data.uri) return null;
    const kind = kindOf(data.uri);
    let subtitle = "";
    if (kind === "playlist") subtitle = data.ownerV2?.data?.name ? "Playlist · " + data.ownerV2.data.name : "Playlist";
    else if (kind === "album") subtitle = [data.type ? data.type[0] + data.type.slice(1).toLowerCase() : "Album", artistNames(data.artists)].filter(Boolean).join(" · ");
    else if (kind === "artist") subtitle = "Artist";
    else if (kind === "show") subtitle = "Podcast · " + (data.publisher?.name || "");
    else if (kind === "collection") subtitle = "Playlist";
    else if (kind === "track") subtitle = artistNames(data.artists);
    return {
      uri: data.uri,
      kind,
      name: data.name || data.profile?.name || "",
      subtitle,
      image: imageOf(data),
    };
  };

  const track = (data, uid) => {
    if (!data || !data.uri || kindOf(data.uri) !== "track") return null;
    return {
      uri: data.uri,
      uid: uid || "",
      name: data.name || "",
      artists: artistNames(data.artists || data.firstArtist),
      album: data.albumOfTrack?.name || "",
      albumUri: data.albumOfTrack?.uri || "",
      image: imageOf(data.albumOfTrack || data),
      durationMs: data.duration?.totalMilliseconds || data.trackDuration?.totalMilliseconds || 0,
      explicit: data.contentRating?.label === "EXPLICIT",
      playable: data.playability ? data.playability.playable !== false : true,
    };
  };
  const unwrap = (w) => (w && (w.data || w.item?.data || w.itemV2?.data)) || w;

  const api = {
    async library() {
      const data = await query("libraryV3", {
        filters: [],
        order: null,
        textFilter: "",
        features: ["LIKED_SONGS", "YOUR_EPISODES", "PRERELEASES"],
        limit: 200,
        offset: 0,
        flatten: false,
        expandedFolders: [],
        folderUri: null,
        includeFoldersWhenFlattening: true,
      });
      const items = (data?.me?.libraryV3?.items || [])
        .map((entry) => {
          const d = entry.item?.data;
          if (!d) return null;
          if (d.__typename === "PseudoPlaylist") return { uri: d.uri || "spotify:collection:tracks", kind: "collection", name: d.name || "Liked Songs", subtitle: (d.count || 0) + " songs", image: imageOf(d) };
          return item(d);
        })
        .filter(Boolean);
      return { items };
    },

    async me() {
      const data = await query("profileAttributes", {});
      const p = data?.me?.profile || {};
      return { name: p.name || p.username || "", username: p.username || "", image: pickImage(p.avatar?.sources, 64) };
    },

    async home() {
      const cookie = (name) => (document.cookie.match(new RegExp("(?:^|; )" + name + "=([^;]*)")) || [])[1] || "";
      const data = await query("home", {
        includeHomeChapterVideoCards: false,
        homeEndUserIntegration: "INTEGRATION_WEB_PLAYER",
        includeEpisodeContentRatingsV2: false,
        timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone,
        sp_t: cookie("sp_t"),
        facet: "",
        sectionItemsLimit: 12,
      });
      const sections = (data?.home?.sectionContainer?.sections?.items || [])
        .map((s) => ({
          title: s.data?.title?.transformedLabel || s.data?.title?.text || "",
          items: (s.sectionItems?.items || []).map((i) => item(unwrap(i.content))).filter(Boolean),
        }))
        .filter((s) => s.title && s.items.length);
      return { greeting: data?.home?.greeting?.transformedLabel || "", sections };
    },

    async search(term) {
      const data = await query("searchDesktop", {
        searchTerm: term,
        offset: 0,
        limit: 10,
        numberOfTopResults: 5,
        includeAudiobooks: false,
        includeArtistHasConcertsField: false,
        includePreReleases: true,
        includeAlbumPreReleases: false,
        includeLocalConcertsField: false,
        includeAuthors: false,
        includeEpisodeContentRatingsV2: false,
      });
      const s = data?.searchV2 || {};
      const items = (list) => (list?.items || []).map((i) => item(unwrap(i))).filter(Boolean);
      return {
        tracks: (s.tracksV2?.items || []).map((i) => track(unwrap(i.item || i))).filter(Boolean),
        artists: items(s.artists),
        albums: items(s.albumsV2),
        playlists: items(s.playlists),
      };
    },

    async playlist(uri, offset = 0) {
      const data = await query("fetchPlaylist", { uri, offset, limit: 100, enableWatchFeedEntrypoint: false });
      const p = data?.playlistV2 || {};
      const tracks = (p.content?.items || []).map((i) => track(unwrap(i.itemV2), i.uid)).filter(Boolean);
      return {
        header: { uri, kind: "playlist", name: p.name || "", subtitle: (p.ownerV2?.data?.name || "") + (p.content?.totalCount ? " · " + p.content.totalCount + " songs" : ""), image: imageOf(p), description: (p.description || "").replace(/<[^>]+>/g, "") },
        tracks,
        total: p.content?.totalCount || tracks.length,
      };
    },

    async album(uri) {
      const data = await query("getAlbum", { uri, locale: "", offset: 0, limit: 50 });
      const a = data?.albumUnion || {};
      const image = imageOf(a);
      const tracks = (a.tracksV2?.items || a.tracks?.items || [])
        .map((i) => {
          const t = track(i.track, i.uid);
          if (t) {
            t.album = a.name || "";
            t.albumUri = uri;
            t.image = t.image || image;
          }
          return t;
        })
        .filter(Boolean);
      const year = a.date?.isoString ? a.date.isoString.slice(0, 4) : a.date?.year || "";
      return {
        header: { uri, kind: "album", name: a.name || "", subtitle: [artistNames(a.artists), year, tracks.length + " songs"].filter(Boolean).join(" · "), image, description: "" },
        tracks,
        total: a.tracksV2?.totalCount || tracks.length,
      };
    },

    async artist(uri) {
      const data = await query("queryArtistOverview", { uri, locale: "", includePrerelease: true });
      const a = data?.artistUnion || {};
      const tracks = (a.discography?.topTracks?.items || []).map((i) => track(i.track, i.uid)).filter(Boolean);
      const releases = (a.discography?.popularReleasesAlbums?.items || a.discography?.albums?.items || [])
        .map((r) => item(r.releases?.items?.[0] || r))
        .filter(Boolean);
      const followers = a.stats?.monthlyListeners ? a.stats.monthlyListeners.toLocaleString() + " monthly listeners" : "";
      return {
        header: { uri, kind: "artist", name: a.profile?.name || "", subtitle: followers, image: imageOf(a.visuals) || pickImage(a.visuals?.avatarImage?.sources), description: "" },
        tracks,
        releases,
        total: tracks.length,
      };
    },

    async likedSongs(offset = 0) {
      const data = await query("fetchLibraryTracks", { offset, limit: 100 });
      const t = data?.me?.library?.tracks || {};
      const tracks = (t.items || []).map((i) => track(unwrap(i.track), i.track?._uri)).filter(Boolean);
      return {
        header: { uri: "spotify:collection:tracks", kind: "collection", name: "Liked Songs", subtitle: (t.totalCount || tracks.length) + " songs", image: "", description: "" },
        tracks,
        total: t.totalCount || tracks.length,
      };
    },
  };

  // --- The player: its state ----------------------------------------------

  const $ = (selector, root = document) => root.querySelector(selector);
  const testid = (id, root) => $(`[data-testid="${id}"]`, root);
  const clockMs = (text) => {
    if (!text) return 0;
    const parts = text.trim().split(":").map(Number);
    if (parts.some(isNaN)) return 0;
    return parts.reduce((acc, p) => acc * 60 + p, 0) * 1000;
  };
  const media = () => [...document.querySelectorAll("video, audio")].find((m) => m.duration > 0 || m.src) || null;

  const playerState = () => {
    const bar = testid("now-playing-bar") || document;
    const meta = navigator.mediaSession && navigator.mediaSession.metadata;
    const playPause = testid("control-button-playpause", bar);
    const shuffle = testid("control-button-shuffle", bar);
    const repeat = testid("control-button-repeat", bar);
    const titleLink = $('[data-testid="context-item-info-title"] a, [data-testid="context-item-link"]', bar);
    const contextLink = $('[data-testid="now-playing-widget"] a[href*="/playlist/"], [data-testid="now-playing-widget"] a[href*="/album/"]', bar);
    const position = testid("playback-position", bar);
    const duration = testid("playback-duration", bar);
    const range = $('[data-testid="playback-progressbar"] input[type="range"]', bar);
    const volume = $('[data-testid="volume-bar"] input[type="range"]', bar);
    const mute = testid("volume-bar-toggle-mute-button", bar);
    const like = $('[data-testid="now-playing-widget"] button[aria-checked]', bar);
    const m = media();
    const artwork = meta && meta.artwork && meta.artwork.length ? meta.artwork[meta.artwork.length - 1].src : "";
    const playLabel = (playPause && playPause.getAttribute("aria-label")) || "";
    const trackHref = titleLink ? titleLink.getAttribute("href") || "" : "";
    const repeatLabel = (repeat && repeat.getAttribute("aria-label")) || "";
    const repeatChecked = repeat && repeat.getAttribute("aria-checked");
    return {
      signedIn: !testid("login-button"),
      title: (meta && meta.title) || (titleLink && titleLink.textContent) || "",
      artist: (meta && meta.artist) || "",
      album: (meta && meta.album) || "",
      artwork,
      trackUri: trackHref.includes("/track/") ? "spotify:track:" + trackHref.split("/track/")[1].split(/[?#]/)[0] : "",
      contextHref: contextLink ? contextLink.getAttribute("href") || "" : "",
      // The player's own button says what a press would do.
      playing: m ? !m.paused : /pause/i.test(playLabel),
      canPlay: !!playPause && !playPause.disabled,
      positionMs: range && range.max ? Math.round(+range.value) : clockMs(position && position.textContent),
      durationMs: range && range.max ? Math.round(+range.max) : clockMs(duration && duration.textContent),
      shuffle: !!shuffle && shuffle.getAttribute("aria-checked") === "true",
      shuffleAvailable: !!shuffle && !shuffle.disabled,
      repeat: repeatChecked === "mixed" || /disable repeat/i.test(repeatLabel) ? "track" : repeatChecked === "true" ? "context" : "off",
      volume: volume ? Math.round(+volume.value * 100) / (volume.max > 1 ? +volume.max : 1) / 100 : 1,
      muted: !!mute && /unmute/i.test(mute.getAttribute("aria-label") || ""),
      liked: !!like && like.getAttribute("aria-checked") === "true",
      canNext: !!testid("control-button-skip-forward", bar) && !testid("control-button-skip-forward", bar).disabled,
      canPrevious: !!testid("control-button-skip-back", bar) && !testid("control-button-skip-back", bar).disabled,
      ad: !!$('[data-testid="context-item-info-ad-subtitle"], [data-testid="ad-link"]', bar) || /advertisement/i.test((meta && meta.title) || ""),
      // Free accounts show an upgrade button; Premium ones do not. Unknown
      // until signed in (then false only when the button is truly absent).
      premium: !testid("login-button") && !$('[data-testid="upgrade-button"], a[href*="/premium"], button[aria-label*="Premium" i]', document),
    };
  };

  const report = () => {
    const s = playerState();
    // The position moves every second: post it, but not other noise.
    const json = JSON.stringify(s);
    if (json !== state.last) {
      state.last = json;
      post({ type: "player", state: s });
    }
  };
  setInterval(report, 500);

  // --- The player: its controls -------------------------------------------

  const press = (element) => {
    if (!element) return false;
    element.click();
    return true;
  };
  // React keeps a range's value itself: set it through the native setter so the
  // input event carries the new value.
  const setRange = (input, value) => {
    if (!input) return false;
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
    setter.call(input, String(value));
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
    input.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
    input.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
    return true;
  };

  const pathOf = (uri) => {
    const [, kind, id] = (uri || "").split(":");
    if (kind === "collection") return "/collection/tracks";
    if (kind === "user" && uri.endsWith(":collection")) return "/collection/tracks";
    return "/" + kind + "/" + id;
  };
  const navigate = async (path) => {
    if (location.pathname !== path) {
      history.pushState({}, "", path);
      window.dispatchEvent(new PopStateEvent("popstate", { state: {} }));
    }
  };
  const mainScroller = () =>
    $('[data-testid="playlist-tracklist"]')?.closest("[data-overlayscrollbars-viewport], .main-view-container__scroll-node") ||
    $(".main-view-container [data-overlayscrollbars-viewport]") ||
    $(".main-view-container__scroll-node");

  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  const controls = {
    toggle: () => press(testid("control-button-playpause")),
    play: () => (playerState().playing ? true : press(testid("control-button-playpause"))),
    pause: () => (playerState().playing ? press(testid("control-button-playpause")) : true),
    next: () => press(testid("control-button-skip-forward")),
    previous: () => press(testid("control-button-skip-back")),
    shuffle: () => press(testid("control-button-shuffle")),
    repeat: () => press(testid("control-button-repeat")),
    like: () => press($('[data-testid="now-playing-widget"] button[aria-checked]')),
    mute: () => press(testid("volume-bar-toggle-mute-button")),
    seek: (ms) => setRange($('[data-testid="playback-progressbar"] input[type="range"]'), Math.max(0, Math.round(ms))),
    volume: (v) => {
      const input = $('[data-testid="volume-bar"] input[type="range"]');
      if (!input) return false;
      const max = +input.max || 1;
      return setRange(input, Math.min(1, Math.max(0, v)) * max);
    },

    // Plays a playlist, album, artist or Liked Songs from the start, or from
    // the track at `index` of it.
    async playContext(uri, index) {
      await navigate(pathOf(uri));
      if (index == null || index < 0) {
        const button = await waitFor(() => $('[data-testid="action-bar-row"] [data-testid="play-button"]'));
        if (!button) throw new Error("no play button on " + uri);
        if (/pause/i.test(button.getAttribute("aria-label") || "")) return true; // already playing it
        button.click();
        return true;
      }
      // Rows are virtualized: scroll until the row is made, then play it.
      const rowIndex = index + 2; // the header row is 1
      const findRow = () => $(`[data-testid="tracklist-row"]`) && $(`[role="row"][aria-rowindex="${rowIndex}"] [data-testid="tracklist-row"]`);
      await waitFor(() => $('[data-testid="tracklist-row"]'));
      let row = findRow();
      for (let tries = 0; !row && tries < 60; tries++) {
        const scroller = mainScroller();
        if (!scroller) break;
        const rows = [...document.querySelectorAll('[role="row"][aria-rowindex]')].map((r) => +r.getAttribute("aria-rowindex"));
        const last = Math.max(...rows);
        const first = Math.min(...rows);
        scroller.scrollTop += rowIndex > last ? 56 * Math.max(4, rowIndex - last) : rowIndex < first ? -56 * Math.max(4, first - rowIndex) : 0;
        await sleep(120);
        row = findRow();
      }
      if (!row) throw new Error("row " + index + " not found on " + uri);
      row.scrollIntoView({ block: "center" });
      await sleep(50);
      const button = $('button[data-testid="play-button"], button[aria-label^="Play"]', row);
      if (button) button.click();
      else row.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
      return true;
    },

    // Plays one track on its own page (its radio follows it).
    async playTrack(uri) {
      await navigate(pathOf(uri));
      const button = await waitFor(() => $('[data-testid="action-bar-row"] [data-testid="play-button"]'));
      if (!button) throw new Error("no play button on " + uri);
      button.click();
      return true;
    },
  };

  window.__spotish = {
    // The app calls these with Runtime.evaluate; each answers a JSON string.
    async call(name, args) {
      try {
        let value;
        if (api[name]) value = await api[name](...(args || []));
        else if (controls[name]) value = await controls[name](...(args || []));
        else if (name === "state") value = playerState();
        else if (name === "query") value = await query(args[0], args[1]);
        else throw new Error("unknown call " + name);
        setTimeout(report, 150);
        return JSON.stringify({ ok: true, value: value === undefined ? null : value });
      } catch (error) {
        return JSON.stringify({ ok: false, error: String((error && error.message) || error) });
      }
    },
    signedRequests: () => !!state.headers,
  };
})();
