# Keeping uQ, Bindle and the apps compatible

uQ, Bindle and the apps made for uQ are separate packages on the AUR, released on their own. They
still use each other: Settings pages from every app load into uQ, uQ's bar asks oysterAV and uQbit
for their state, Bindle runs `uqctl`, hauler and appOFholding hand files to `oyst-cli scan`. An
update must never install a set of versions where one of these breaks. Three things make sure:

1. **Every package says which versions of its partners it works with**, as pacman relations in its
   PKGBUILD. pacman then refuses a set that would break, whatever installs it.
2. **bindleUP holds back an update whose partner isn't ready**, and installs partners that update
   together in the order pacman needs, so a run never stops half way.
3. **CI checks the latest releases together**, every day, so a release that would hold others back
   or break them shows up before anyone installs it.

## The manifest: `compat/family.toml`

One file in uQ lists the family: each package (its repo, its AUR packages, the PKGBUILDs they
are built from) and the **contracts** between them. A contract is one thing a package provides
that others use, with a name, a provider and **levels**:

| Contract | Provider | What |
|---|---|---|
| `uq-api` | uq | integration files: pages, widgets, window rules, launcher providers, app actions (`uq_api`) |
| `uq-theme` | uq | `tokens.json` and the KDE colour scheme, as the uq-theme kit reads them |
| `uq-diag` | uq | the `[CODE]` log lines, problem-code lists and crash records uQ reads |
| `uq-desktop` | uq | uqd's D-Bus API (`org.uq.Desktop1`, launcher providers) |
| `uqctl` | uq | the `uqctl` commands apps run |
| `bindle-cli` | bindle | `bindle snapshot`, `recovery --status`, `rollback`, `bindleup --gui`, … |
| `oyst-scan` | oysterav | `oyst-cli scan … --ndjson` and its exit codes |
| `oyst-status` | oysterav | `oyst-cli status summary --deep --json` |
| `uqbit-status` | uqbit | `uqbit status --json` |
| `uqview-cli` | uqview | `uqview -- PICTURE`, `uqview --markup PICTURE` |
| `kyko-cli` | kyko | kyko as the terminal, `kyko toggle` |

`levels.N` is the first provider release that speaks level N. A provider keeps every level it has
spoken until it **drops** one (`dropped.N`, the first release without it). Adding to a contract in
a way old users don't notice keeps the level; anything an old user can't cope with (a removed
command, a renamed field, a changed exit code) is a new level, and the old one is kept for a while.

Each package lists what it `needs`: `needs = { uq-api = 3, oyst-scan = 1 }`. `min` raises the floor
for one provider when the package uses something newer than the level's first release
(`min = { uq = "0.3.45" }`).

## The PKGBUILD line

`compat/compat.py` turns the manifest into one line in each PKGBUILD:

```sh
conflicts+=('oysterav<0.5.0' 'uq<0.3.41')  # uq-compat: from uQ's compat/family.toml
```

- **Floors**: for each provider a package needs, `provider<floor` (the highest floor of the levels
  it needs). Only installed packages conflict, so an app still installs without its partners;
  with an older partner, pacman wants the partner updated in the same transaction.
- **Ceilings**: once a level the package needs is dropped, `provider>=dropped`.
- **Old users**: a provider that dropped a level gets `user<since` for each package that moved on
  (`since = { oyst-scan = "0.3.0" }` in the user's entry: its first release on the newer level).
  A provider can't drop a level while some package's latest release still needs it.

Don't edit the line by hand. To change it, change `family.toml`, then from a folder with every
repo checked out:

```sh
python3 compat/compat.py sync ~/Projects      # rewrites the line in every PKGBUILD
python3 compat/compat.py check --repos ~/Projects
```

and open a pull request in each repo whose line changed. It goes out with each package's next
release. `compat.py line hauler` prints one package's line.

## bindleUP

Before the AUR stage, bindleUP reads each update's new `conflicts` and `depends` from the AUR's
info API and the installed AUR packages' from `pacman -Qmi`, and works out the set as it will be
(Bindle's `crates/bindle-upgrade/src/compat.rs`):

- an update whose versioned relations can't be met waits (`paru -Sua --ignore hauler`), and says
  why: `held back: hauler 0.2.6-1 doesn't work with oysterav 0.5.0-1 (oysterav<0.6.0); it waits for
  oysterav 0.6.0`;
- an update that would break an installed package with no update for it yet waits too;
- what depends on a held update waits with it; everything else updates;
- when one update needs another's new version, they go in runs in that order (`paru -S --needed --
  uq`, then `-- hauler`), and two that need each other go in one `--batchinstall` transaction.

If the AUR can't be asked, nothing is held, and pacman still refuses a set that clashes.

## CI: `.github/workflows/family.yml`

Daily, and on every change to `compat/`:

- `compat.py`'s tests and the manifest's own rules;
- every repo at main: its PKGBUILDs carry the line `family.toml` gives, and what its files use
  (the newest `uq_api` of its integration files, the uq-theme and uq-diag kits in
  `uq/native.toml`) is what `family.toml` says it needs;
- every repo at its latest release: each package's needs are met by its providers' latest releases;
- the AUR: every package's conflicts can be met by the AUR's own versions, and which packages are
  behind their release (said, not failed: the AUR is pushed by hand);
- every app's integration files at its latest release load in `uqctl` from uQ at the floor
  (`uq-api`'s first level), at uQ's latest release and at main.

## When you change a contract

- **Adding** (a new command, a new JSON field old users ignore): nothing to do, beyond raising a
  user's `min` when it starts relying on the addition.
- **Breaking** (removing or renaming something, changing what it means): add level N+1 with the
  release that brings it, keep level N working, and move users over one by one (`needs`, `since`).
  Once no package's latest release needs level N, add `dropped.N` with the release that removes it.
- **A new contract**: add it with `levels = { 1 = "<the provider's current release>" }` and add it
  to the `needs` of every package that uses it.
