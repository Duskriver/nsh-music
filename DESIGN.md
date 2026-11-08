# Spotish design rules

The rules Spotish's interface keeps, collected from design reviews. They
apply on top of the [NSH Design Guides](https://nsh.dev/docs/design-guides.md);
where the two differ, these win for Spotish. Change them here first, then in
the code.

## Identity

- Spotish has its own look. It is not a Spotify clone and does not borrow
  Spotify's green for its controls.
- Dark and quiet. The window, the pages and Now Playing share the sidebar's
  near-black (`bg-sidebar`). No grey content panel. NSH's Linux NavStack
  paints its pages `#242424`, so every page is an `AppPage` (`src/components/page.tsx`), which covers that with the app's colour.
- No decorative gradients. In particular, no colour wash at the top of a
  page or of Now Playing: they read as dirty. The only colour that moves is
  the music light (below).

## Colour

- Theme colours come from tokens (`theme.json`, `src/platform/palette.ts`),
  never literals in components.
- Player controls are white (`foreground`), never solid: the play button,
  the seek and volume ranges, toggles that are on. Never
  green, never the cover's colour. A white control reads over any cover;
  a coloured one clashes (red cover, green button).
- Frosted, not solid: the player bar's white controls are translucent white
  with a fine border, so the light behind shows through.
- The play button over pictures (cards) is dark: a near-black, slightly
  translucent disc with a light glyph and a fine light border, which reads
  over any cover, light or dark.
- Liked is red (`liked` token), with a white heart on Liked Songs' cover.
- The like button is always the filled heart: white (dimmed until hovered)
  while not liked, red once liked. Never an outline.
- Menus and popovers are a solid, dark surface (`popover`, a step above the
  window), not a pale grey.
- The light menu is always the last control on the right, after the volume
  (the bar and Now Playing alike).
- A desktop app's window keeps the desktop layout at any size; phones, and
  a browser page narrower than 640pt, get the phone's (tabs, mini player).
  Narrower than 860pt the sidebar is a rail. The header's first button
  folds the sidebar away entirely (the pages take the whole window; the
  user's picture comes to the header's top-left corner, opening the same
  user menu downward) and brings it back, at any width (remembered between runs; it animates like
  a breakpoint).
- Card grids (an artist's discography, search) fill their row: as many
  columns as fit at about 144pt, the cards widened to share the row out,
  the play button keeping its golden proportion to the cover. The player bar follows the pages' width (the window less the
  sidebar): narrower than 760pt it is compact, one row, left and
  right: the cover, title and Like on the left; the transport and the light
  menu on the right; no position or volume (Now Playing has both). Narrower
  than 520pt it is stacked: taller, two rows (a 36pt radius): the cover,
  the title and artist centred, and Like; under them the transport centred
  and the light menu at the end; under 560pt shuffle and repeat go too. The
  header's Premium becomes an icon under 640pt.
- Crossing one of these breakpoints animates (`animateLayoutOn`): the
  sidebar and the bar glide to their new size and place, what comes or
  goes fades (240ms, ease in-out). Resizing between breakpoints follows the
  window at once; reduced motion jumps.
- A card's play button is in golden proportion: the 124pt cover is φ³ times
  the button (29pt), the button φ² times its inset (11pt). Play buttons
  have no border, only a soft shadow.
- The header ends, on a desktop app, with a close button (✕, "Close
  window"): it closes the main window as the title bar's close does
  (hidden while the tray keeps the app; the music plays on).
- Exit ends the app: the last row of the user menu (after Log out) and of
  the tray menu.
- The desktop sidebar is 274pt wide, 228pt in a middle window (860 to
  1100pt), a 72pt rail below that. The account menu is exactly as wide as
  the account row (the sidebar less its panel's padding and border), so it
  opens flush with it, edge to edge.
- The account menu's rows have an icon, centred in a slot as wide as the
  avatar above (32pt), so icons line up under the avatar's centre and
  labels under the name; a page that opens in the browser ends in ↗.
- The light comes from the cover. Extract a palette from it (up to eight
  colours, `coverColorsQuery`); each effect declares how many colours it uses
  and repeats them when the cover has fewer.
- Its brightness is controlled by an algorithm (`tame()` in
  `src/components/ui/toolbar-fx.tsx`): the hue is kept, the luminance is held
  near 0.32. The light must never be bright enough to hurt the contrast of
  the controls over it.
- It moves with the music, with crests: louder bands rise as peaks, the beat
  swells them. Every effect must have music-driven peaks. Edges are smooth
  (no hard top edge).
- Fourteen effects, chosen from the ✦ menu on the player bar and in Now
  Playing ("Music light": one numbered line per effect, 01 to 14, the one in
  use marked with a check on the right, scrolling when taller than the
  room; remembered, the same one in both). Dots, Grid and Wave first (the
  best; Dots the default), then Orb (a sphere fully covered in dots,
  turning, its surface pushed out by the spectrum), Spectrum, Bars, Ribbons, Halo (a wave
  running into a smooth ring), Aurora (rays under a crisp hem), Mountains
  (many fine ridgelines in perspective), Mesh (the same terrain as a net of
  rows and columns), Particles (a wave of dots in perspective) Strings
  (vibrating strings) and Rhythm (a rhythm game's note highway as wide as the surface,
  lanes at a piano key's pitch: white notes, short and held, each a stack
  of short segments, fall to the bottom. The rhythm shoots them: on each
  beat a handful of the loud lanes fire a short shot at their lowest note
  once it is close, all the light kept low (a muzzle flash, a short bolt, a
  flash at the note's lower edge, sparks flying on the shot's way: up
  and out past the note's sides, never back down; all the light (the
  shot, its flashes, the sparks) is in the lane's colour, the notes stay
  white); the note is knocked back up a little
  and settles; it keeps its white and only brightens a little, segment
  by segment up from the hit, never all at once. A
  different handful each beat, never all of them; crisp, never blurred;
  each shot plays out over about half a second (the
  lights get the time since the last beat for this, `since`, and that
  beat's own seed, `onset`); its light is the cover's hue kept vivid). Lines are crisp: a thin bright core in a soft glow, never
  only a blur. Their flow comes from a clock read with the music's
  levels, never `nsh.time`.
- Each effect has its own height in Now Playing (the terrains and the
  aurora fill the room, the lines take about half), and never reaches past
  the top of the strip it is drawn in.
- Grid's blocks are sized to the surface (small in the player bar, larger
  in Now Playing) and snapped to whole device pixels at any zoom.
- It rests when nothing plays and when the user asks for less motion.
- Nothing may redraw every display frame without need. A shader that reads
  `nsh.time` repaints the whole window at the display's rate, so pause its
  clock whenever the music's levels drive it instead. The levels reach the
  visuals at about 60 frames a second (30 visibly stutters).
- Keep it cheap. Draw only where the light is (e.g. the bottom strip of Now
  Playing), never sample the backdrop on a full screen, and pause what is
  hidden. Now Playing must hold the display's refresh rate.

## The player bar (desktop)

- A pill floating over the right-hand pages, the sidebar fixed on the left.
  The pages scroll under it.
- A solid surface in the player colour, the music light inside. Not a
  gradient, not a sheen, and no glass: it does not read the pages behind
  it, which would cost every scroll frame a picture of them.
- Its border is a single 1px line that catches light. No glow on the border.
- The controls are quiet, here as in Now Playing: the light leads. A faint
  play disc (no border or shadow), thin faint sliders.
- The volume's slider always shows in the player bar (about 96pt),
  balancing what plays on the left; nothing hides or slides. Its
  thumb stays inside the bar at 0 and at 100%.

## Now Playing

- Full screen, the controls laid directly on the page. No floating card or
  glass panel.
- The music light is the page's background and its lead, rising from the
  bottom up to nearly half the window. The controls stay quiet so they
  don't compete with it: a long, thin, faint position bar; a faint play
  disc with no border or shadow; smaller transport icons; the volume
  collapsed until hovered.
- Now Playing is responsive: the cover takes the room the window leaves
  (up to 320pt). Tall, it sits above the words and controls; wide and short
  (640pt or wider, under 560pt tall), it sits beside them; with too little
  room for a 96pt cover it gives way, and the words and controls stay.
- Now Playing closes from a down chevron at the top left: it goes back
  down into the bar, not "back" to a page. In the
  bar, the whole left block (cover, title, artist) opens it.
- Words that are only read can be selected and copied: Now Playing's title
  and artist, a collection's header, About. Rows and buttons are not.
- The volume sits in the top-right corner beside the light menu, always
  open (no hover reveal), out of the music's way.

## Controls

- Sliders: the range and the track share one centre line, and no thumb dot
  is drawn (the bar itself is the control). If a thumb ever returns, never
  scale it: NSH centres it with a transform that a scale replaces.
- The volume bar is short (about 64pt).
- A slider keeps one height (it doesn't grow on hover).
- Nested radii are concentric: inner radius = outer radius − gap
  (`--radius-card`, `--radius`, `--radius-inset`).
- Lists have a small gap between rows (4pt).
- Rows in one surface share one geometry: in the sidebar, every row (Home,
  Search, the library) is 44pt tall, padded 8pt, radius `--radius-inset`,
  its icon or cover in the same 30pt column, so labels line up. A panel's
  inset is the same on every side (8pt: a hovered row keeps clear of the
  border), and its radius is the row's plus that inset (16 = 8 + 8).
- Times (position, duration, a track's length) are monospace, equal-width
  figures (`Digits`, `src/components/ui/digits.tsx`), so they don't jitter.
  The monospace face is JetBrains Mono where installed, else the system's
  own (`ui-monospace`, then `monospace`): theme.json's `font-mono` is a
  fallback list, so no system warns about a missing font.

## Motion

- Every interactive control has layered hover and press feedback: a
  translucent wash, a press, and its icon moving once.
- Buttons move like affectui's clean button: hover only brings a wash (or a
  shade, a slightly deeper shadow) in, 120ms ease-out, never a grow; a press
  takes the whole button to 0.97 (an icon button's disc and glyph to 0.95)
  over 180ms on the smooth-out curve (0.22, 1, 0.36, 1), and the wash
  deepens. A labelled button's new label rises 4pt as it fades in (360ms).
- A glyph that changes (play and pause, the heart, repeat, the volume's
  levels, mute) swaps like transitions.dev's "Icon swap" (`SwapIcon`): the
  old one shrinks to 0.25, blurs to 2px and fades as the new one grows from
  0.25 and sharpens, together, 240ms ease-in-out. NSH can't animate a blur,
  so it steps 4 times through the swap; nothing runs at rest. The hover
  morph stays on the glyph showing. A glyph that only changes colour (the
  like heart turning red, a toggle turning on) fades the old colour off over
  180ms ease-out instead of snapping.
- Icon hover motion is subtle and plays once per entry (lift, swing, nudge,
  beat, turn), modelled on allsum-nsh's icon motion. No big presets.
- Layers move separately, and hover is steady: a card doesn't lift or
  change size; a wash comes in, the cover grows a hair (1%), and the play
  button rises and grows over it.
- Pages and their items enter with a short stagger; dialogs rise and fade
  in; toasts slide in and out.
- Everything that loads (home, a collection and an artist's Popular, search
  results, the library) follows transitions.dev's "Skeleton loader and
  reveal" (`Reveal` in `ui/feedback.tsx`): a skeleton shaped like its rows or
  cards pulses (each shape to half opacity and back, 1s ease-in-out), then
  fades out as the content fades in over it, 360ms ease-in-out. The
  reference's 2px cross-blur is left out (NSH can't animate a filter).
  Content already cached shows at once, without a skeleton.
- List rows (tracks in a collection, an artist's Popular and search; the
  library) enter in an irregular wave, top to bottom: each fades in moving
  right to left (NSH's `fadeInRight`), passing its place by 1pt before it
  settles, on the smooth-out curve. Turns are 55ms apart (the first 12 rows
  in view; any below come in with the 12th), and each row adds its own fixed
  lag of up to 90ms and travels 12-20pt over about 405-475ms, so neighbours
  can trade places while the list still flows downward. The scatter is a
  hash of the row's place: the same on every render, never reshuffled.
  Cards (home's rows, search's rows, an artist's releases) wave the same
  irregular way left to right with NSH's `fadeInUp` (rising), the first 8
  taking turns; home's sections start 55ms apart down the page. Rows and
  cards share one stagger (`WAVE`, `createListWave`, `listItemEntrance` in
  `ui/motion.ts`). In both, each cover flips open at its own pace (NSH's
  `flipInY`, its left edge swinging in from -90°, past by 8° before it
  settles, under a 600pt perspective), starting its own 120-220ms after its
  row or card and turning over its own 330-550ms.
- The wave plays every time a page is entered: opened, gone back (or
  forward) to in the stack, or shown again from a cached page; content that
  loaded starts its wave as the skeleton is half gone, and a new search's
  results wave in as they replace the old. An item that comes into view
  later (scrolled down to, scrolled across in a card row) comes in on its
  own with the same motion and only a little jitter (up to 60ms). Each item
  comes in once per entry: a recycled cell given an item already shown, a
  re-render or a playback tick plays nothing.
- The sidebar's selection is transitions.dev's "Tabs sliding": rows don't
  light their own background (hover still washes them); each panel has one
  background that slides to the selected row (240ms ease-in-out). In the
  library it sits behind the recycling list at the selected item's place
  (8 + 48 x its index), follows the scroll natively without easing, and is
  clipped by the panel when scrolled away. With no selection in a panel (a
  library page while Home's panel shows, or a page not in the library) it
  fades away; moving between panels, one fades out as the other fades in
  where it lands.
- Menus (the light menu, the account menu) open with transitions.dev's
  "Dropdown menu morph", from `ui/menu.ts`: the popup grows out of the
  trigger's nearest corner, slid 8pt toward it, at 0.97 scale and 2px blur,
  fading in over 360ms on a slight overshoot; it leaves the same way in
  240ms without one.
- Navigation direction follows history: returning to a page already in the
  stack animates back, a page ahead animates forward.
- Liking is transitions.dev's "Like button": the heart squashes to 0.82 and
  springs back past its size (360ms, overshoot curve) while 8 red dots spray
  out about 20pt, each with its own angle, reach, size and delay (about
  600ms). It plays only when the user's own press turns Like on; unliking
  just dips the heart (0.95, 180ms). Nothing plays on track change.
- Everything respects reduced motion.

## Icons and logo

- Icons are [Tabler Icons](https://tabler.io/icons) (MIT): Outline drawn
  at a 1.5pt stroke, with Filled for the transport and the liked heart. One
  family across the app; a missing glyph comes from Tabler too, never drawn
  by hand.
- Icon hover motion follows the glyph's own shape, never a generic move
  pasted on: skip leaves an echo the way it goes, shuffle's lines cross,
  repeat's arrows turn once, the volume's waves come in one by one, the heart
  beats. Small and once per entry.
- The logo is the website's mark: three rounded bars of a level meter on a
  glossy green tile (#5ff08f to #14833b). The app (`src/ui/logo.tsx`), the
  favicon and the website's header draw the same mark; the tray draws the
  three bars alone, in the panel's text colour. Beside a title and its
  subtitle it is as tall as both lines (44pt).

## Layout and density

- Compact. Don't waste rows: a collection's header is a small cover, the
  title and an inline play button on one band.
- Sidebar library rows are one line, with small round covers (Liked Songs
  round too) and the playing indicator on the current row.
- No title above the library list; the panel is labelled for screen readers.

## Words

- Short, sentence case. Menu items say only what they are: Premium, Spotify,
  About…, GitHub, NSH, Log out. "…" marks an item that opens a dialog.
- Say "Spotify", not "spotify.com".
- The tray menu has three groups: "Open Spotish" on its own; then what plays
  (its title, Play/Pause, Next track, Previous track, Show Now Playing); then
  Quit Spotish.

## Performance

Spotish is NSH's showcase: it must be fast and light.

- Anything that can grow is a virtualized `List`: the library, a
  collection's tracks, the home page's sections and every row of cards
  (horizontal). Only what is near the view exists.
- Lists of equal items snap as they scroll (`snapToInterval` +
  `snapToAlignment="start"`): a card row stops at a card's start, the
  library at a whole row. Snapped items never touch the edge: a card row
  runs out to the page's edges and insets its cards by the gutter (12pt),
  so every stop leaves that margin. A list under a header of unknown height (a
  collection's tracks) does not snap, or every stop would cut a row.
- Pictures are loaded at the size they show: small covers (up to 40pt) ask
  Spotify for the 64px variant (`src/data/images.ts`).
- Nothing redraws without a change to show (see the music light above).
- Hidden surfaces are unmounted, not just covered (the player bar under Now
  Playing).

## Obligations

These are not style and are never traded away: Spotify's ads play and are
never blocked or skipped, Premium is always offered to free accounts, the
unofficial disclaimer stays, and NSH is credited.
