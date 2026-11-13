# Customisable bar islands — design

Status: proposed, 2026-11-13. Builds on main at 8db1b68.

## What exists

- `shell/bar/Bar.qml` hard-codes five islands in a `RowLayout`: System, Work, a filler, Time
  (clock + weather + calendar popup + city search), Notifications, Status. Each can only be
  turned on or off: `shell.islands.<id>` booleans in the `shell` settings section
  (`crates/uq-core/src/settings/shell.rs`), read by `ShellPrefs.islandOn()`.
- uqd owns settings (`GetSettings`/`SetSection`, `SettingsChanged` to the shell), so a change in
  the `shell` section already applies live.
- The uqctl bridge already does network fetches (`bridge/net.rs`): Open-Meteo weather every
  30 min with backoff, geocoding, the public-IP lookup. 5 s timeout, no redirects, nothing
  logged. The shell gets `Weather` events.
- App widgets (`bridge/widgets.rs`, `shell/widgets/`) are *third-party* QML, sandboxed to the
  D-Bus members their manifest lists, shown as small icons inside the Status island.

**Decision: new islands do not build on the app widget system.** That system exists to fence
off other packages' code (manifest-listed D-Bus only, no network, no settings). Built-in islands
need privileged things it deliberately forbids: settings writes, bridge network fetches, opening
URLs. They are first-party QML under `shell/bar/islands/`, fed by bridge events. (Placing an app
widget as its own island is a possible follow-up; not in this work.)

## Data model

A new field in the existing `shell` section, `shell.bar`:

```toml
[shell.bar]
start  = [{ id = "system" }, { id = "work" }]
center = []
end    = [{ id = "weather" }, { id = "clock" }, { id = "separator" },
          { id = "notif" }, { id = "status" }]

[shell.bar.options]          # per-island options, only for islands that have any
clock   = { seconds = false, date = "short" }       # date: none | short | long
stocks  = { symbols = ["AAPL", "MSFT", "^GSPC"] }
news    = { feeds = ["https://feeds.bbci.co.uk/news/world/rss.xml"], max = 20 }
worldclock = { zones = ["America/New_York"] }
```

- Three zones, `start` / `center` / `end`: a true centre (the clock-in-the-middle layout people
  ask for) that a single list with flexible spacers can't give.
- Each entry: `{ id, size? }`. `size` is `compact | normal | wide` (default `normal`); each
  island decides what that means (clock compact = time only; tickers = 12 / 20 / 32 ×
  `Theme.fontSize` wide). No pixel values in config, so sizes follow the theme.
- `id` is a known island id. Every island at most once across all zones, except `separator`
  (any number, a thin `Theme.border` rule). At most 24 entries.
- Validation in uq-core like the rest of `shell`: strict on `SetSection`, lenient at load
  (unknown ids, duplicates and surplus entries dropped).
- **Migration:** no `shell.bar` → the layout is derived from today's `islands` booleans
  (start: system, work; end: weather if a location is set, clock, notif, status, minus any
  turned off). uqd always sends the effective `bar`, so the shell and Settings never derive it
  themselves. `islands` stays readable for older files; nothing writes it any more.
- `clock_24h` stays where it is (the lock screen uses it too).
- Same layout on every screen (per-monitor bars stay on/off via the existing monitor setting).

## Islands

Built now (asked for):

| id | What | Data |
|---|---|---|
| `weather` | Icon + temperature (+ city when not sharing). Popup: today's high/low, next hours, city search, "Remove from bar". | Open-Meteo, existing loop, extended to hourly/daily. Only the chosen location is sent (rounded to 2 dp, as now). |
| `stocks` | Scrolling ticker: `AAPL 354.88 ▼0.1%`. Popup: one row per symbol (price, change, day range, "as of"), add/remove symbols, reorder. | Yahoo Finance chart endpoint (keyless, unofficial; Stooq's CSV API is gone). One request per symbol, max 12 symbols, every 5 min, only while the island is placed. Up/down use `Theme.success`/`Theme.error` *plus* ▲▼ glyphs, never colour alone. |
| `news` | Scrolling headlines with the source. Popup: newest 20 headlines (title, source, age), Enter/click opens in the browser; feed picker with presets + custom https URL. | RSS 2.0 / Atom parsed in Rust (quick-xml, already a dep). Every 15 min, 512 KiB cap per feed, max 8 feeds. Titles plain text only; links must be http(s). |

Existing ones, now placeable: `system`, `work`, `clock` (was `time`, minus weather), `notif`,
`status`, plus `separator`.

**Proposed extras** (to build after this list is agreed):

| id | What | Data |
|---|---|---|
| `worldclock` | One or two other time zones: `NYC 09:41`. Popup: zone picker. | Local (tz database), no network. |
| `sysmon` | CPU %, memory %, (CPU temp if a hwmon sensor exists). Popup: per-core bars, top memory use, "Open system monitor". | `/proc/stat`, `/proc/meminfo`, hwmon, read by the bridge every 2 s only while placed. |
| `media` | Now playing (scrolling title) with prev/play/next. Today it lives inside Status; this lets people put it anywhere (Status keeps its own unless the island is placed). | MPRIS, as Status does now. |
| `keyboard` | Current layout (`EN`), click to cycle. Only shows with 2+ layouts. | Existing layout state. |

Not proposed: CPU/GPU graphs (busy, costly), calendar events (needs accounts), crypto (same
source as stocks; symbols like `BTC-USD` already work in `stocks`).

## Where things live

- **uq-core** `settings/shell.rs`: `BarLayout`, `BarItem`, `IslandOptions`, validation,
  migration, the list of known island ids with their names (shared with Settings).
- **uqctl bridge**: `bridge/net.rs` keeps weather (gains hourly/daily); new `bridge/feeds.rs`
  for stocks and news, `bridge/sysmon.rs` for the monitor. Each loop watches the settings
  receiver and runs only while its island is in the layout; turning it off stops fetching.
  Events: `Weather`, `Stocks`, `News`, `SysMon`. Each payload carries `status`
  (`ok | stale | error | offline`), `fetched_at`, and the last good data.
  Caching: last good result in memory, and in `$XDG_CACHE_HOME/uq/feeds/{stocks,news}.json`
  so the bar has something at login before the network is up (shown as stale with its age).
  Backoff on failure as weather does now; a network change retries at once.
  New call `OpenLink(url)`: http(s) only, URL must be one the bridge itself sent in `News`
  (so the shell can't be used to open arbitrary links), run through `xdg-open`.
  No personal data in requests: no cookies, fixed generic User-Agent `uq/<version>`, no
  location for stocks/news, and the feeds the user picked are the only hosts contacted.
- **Shell**: `Bar.qml` renders the three zones from `Settings.values.shell.bar` with a
  `Repeater` + `Loader` per entry (`shell/bar/islands/<Id>Island.qml`), centre zone truly
  centred. Pure logic in `shell/logic/barlayout.js` (normalise, move, remove, add) and
  `shell/logic/ticker.js` (formatting). A shared `Marquee.qml` scrolls text: pauses on hover
  and on keyboard focus, stops when `Settings.reduceMotion` (then shows the first item and
  cycles every 8 s with no motion), and pauses while the screen is shared.
- **Direct editing in the bar**: right-click any island → a small menu: *Move left*,
  *Move right*, *Remove from bar*, *Add island…* (submenu of the ones not placed), *Edit bar
  in Settings…*. It sends `SetSection("shell", …)` with the patched layout, so it persists
  and every screen updates live. This matches the existing right-click menus (window buttons,
  tray).
- **Settings** (`apps/uq-settings`): the Shell page's "Bar" section becomes a layout editor:
  one list with three zone headers (Left / Centre / Right). Rows can be dragged (a handle),
  or moved with the existing up/down buttons and Alt+Up/Alt+Down — moving past a header
  changes zone. Each row: name, size combo, options button (stocks symbols, news feeds, clock,
  world clock zones) where there are options, remove. Below: "Add island" combo and "Add
  separator". "Reset to default". Rust side in `pages/shell.rs` (`BarMove`, `BarAdd`,
  `BarRemove`, `BarSize`, `BarOptions` ops, applied to uqd's current layout like pin moves).

## Keyboard and accessibility

- Settings editor: everything reachable by Tab; drag has the button/shortcut equivalent.
- Island popups take focus when opened (as WidgetPopup does): arrows move through lists,
  Enter opens/activates, Delete removes a symbol/feed, Esc closes.
- A new shell keybind is **not** added; existing popups open by click. (Open question below.)
- Every island has an accessible name and a text equivalent of what it scrolls.

## Theming

Colours, fonts, radii, gaps and borders only from `Theme` (`surface`, `surfaceAlt`,
`border`, `text`, `textMuted`, `success`, `error`, `accent`, `radius`, `gapIn`, `gapOut`,
`fontUi`, `fontMono`, `fontSize`); widths as multiples of `Theme.fontSize`. Ticker numbers in
`Theme.fontMono` so they don't jitter. The theming review's contrast checks apply to the
up/down colours (with glyphs as the non-colour cue).

## Delivery

1. **PR A — customisable layout**: model + migration, Bar rendering, clock/weather split,
   right-click menu, Settings editor, separators, tests.
2. **PR B — data islands**: weather popup forecast, stocks, news, `OpenLink`, caching, the
   agreed extras, tests.

Each with Rust tests (validation, migration, fetch/parse against local fakes, backoff, cache),
QML tests (`barlayout.js`, `ticker.js`, Marquee reduced-motion/hover), settings-qml tests (the
editor), goldens where the layout reaches rendered output, lints, a nested-session check, and
CHANGELOG entries.

## Open questions

1. Extras: build all four (`worldclock`, `sysmon`, `media`, `keyboard`), or a subset?
2. Stocks via Yahoo's unofficial endpoint (works keyless today, could break or rate-limit;
   failure shows "Quotes unavailable" and keeps the last values). OK, or prefer a keyed source
   (e.g. Finnhub/Alpha Vantage free tier, user pastes a key)?
3. Default news feeds: none (empty island says "Pick feeds"), or a preset like BBC World on
   first add?
4. Should the default layout for new users change (e.g. add a separator), or stay as today?
