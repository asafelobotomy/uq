# Pages from other apps

Thirteen apps ship a page for uQ Settings from their own repository. A page appears under **Apps**
in Settings when the app's package is installed, within a few seconds and with no restart, and
goes when the package is removed. Without uQ the page files do nothing.

Authors: how a page is written is in [`settings-pages.md`](settings-pages.md). The design and the
reasons behind it are in the
[slice E plan](superpowers/plans/2026-11-11-uq-settings-slice-e-companion-pages.md) and spec §8 of
the [Settings design](superpowers/specs/2026-11-10-uq-settings-enhancements-design.md).

| Page | Id | Repo | What it does | Package | Needs running | Password asked for | Status |
|---|---|---|---|---|---|---|---|
| Scheduling | `scdr` | [scdr](https://github.com/asafelobotomy/scdr) | Shows whether scdr is on, pauses and resumes it, turns each kind of work on or off, edits per-app rules, picks the scheduler, and turns the focus agent on or off | `scdr` | `scdrd` (the page offers **Start scdr** when it's stopped) | Pause, resume, turning a kind of work on or off, rules, scheduler and starting scdr | merged |
| Storage and cleanup | `crumbs` | [crumbs](https://github.com/asafelobotomy/crumbs) | Shows disk use, cleans caches and thumbnails, finds leftovers of removed apps, puts back a cleanup, and uninstalls apps. It never touches appOFholding's AppImages | `crum-bs` | Nothing: it runs `crumbs` when asked | System-wide cleanup (such as the journal), leftovers owned by root, uninstalling a package | merged |
| Notifications | `noti-fi` | [noti-fi](https://github.com/asafelobotomy/noti-fi) | Do not disturb, automatic do-not-disturb (full screen, screen sharing), history length and whether it's kept, notification groups; per-app rules are shown, not edited | `noti-fi` | `notifd` (the page says so when it's stopped) | None | merged |
| Memory and swap | `xzram` | [XZram](https://github.com/asafelobotomy/XZram) | Five tabs (`uq_api` 3): memory and swap in use with pressure and a health check; zram size, compression and priority; kernel tuning (swappiness and four more); swap files (create, resize, remove); snapshots (take, restore, delete, trim, undo the last change). Every change shows its plan and streams progress | `xzram` | Nothing: it runs `xzram` when asked | Any change to zram, tuning, swap files or snapshots | merged |
| Tailscale | `scale-tail` | [scale_tail](https://github.com/asafelobotomy/scale_tail) | Connects and disconnects, picks an exit node, accepts routes, allows local network access, lists devices, shows the sign-in link and signs out | `scale_tail` | `tailscaled` | None. A one-time `sudo tailscale set --operator=$USER` lets a normal user change settings | merged |
| Media player | `reelo` | [reelo](https://github.com/asafelobotomy/reelo) | Makes reelo the default player for video and music, and sets resume, play-next-in-folder, keep-the-screen-on, hardware decoding, subtitles, preferred languages and the library folder its start screen lists | `reelo` | Nothing: it runs `reelo` when asked | None | merged |

Pages added since the first five above (all on `uq_api` 3, each checked by the same workflow):

| Page | Id | Repo | What it covers | Package |
|---|---|---|---|---|
| Security | `oysterav-security` | [oysterAV](https://github.com/asafelobotomy/oysterAV) | First-time setup, status, scans and rootkit checks with progress, history, quarantine, scheduled scans, real-time protection, preferences | `oysterav` |
| AppImages | `appimages` | [appOFholding](https://github.com/asafelobotomy/appofholding) | Updates (with progress), the library and each app's policy, background checks, versions kept, sandbox, oysterAV scans, adding from a link, going back a version | `appofholding` |
| Downloads | `hauler` | [hauler](https://github.com/asafelobotomy/hauler) | Downloads now, adding one, history and how long it's kept, folder and space, leftovers, HTTPS-only, scanning, saved logins | `hauler` |
| Software | `software` | [bindaru](https://github.com/asafelobotomy/bindaru) | Searching and installing from the Arch repositories and the AUR (with the recipe review), installed software, repositories, scans | `bindaru` |
| Game library | `romforge` | [romforge](https://github.com/asafelobotomy/romforge) | A read-only view of the library with simple options; the rest stays in the app | `romforge` / `romforge-bin` |
| Emoji and macros | `emobie` | [emobie](https://github.com/asafelobotomy/emobie) | Settings, macros and deleted macros | `emobie` |
| Bindle | `bindle` | [bindle](https://github.com/asafelobotomy/bindle) | The setup Bindle last applied, bindleUP (check and update with progress), what's new, and what Bindle changed with undo | `bindle` |

The first six packages are optional dependencies of `uq` (`optdepends`); the others come with Bindle. Where a page is changed, the app's
own package carries it, so updating the app updates its page.

## Where the work happens

Each page runs in uQ Settings, but the app does the work. A page can only call the programs and
D-Bus methods its `page.toml` lists, and `uqctl integrations check` verifies that list. The pages
never write `user.lua`.

## Checking them

`.github/workflows/companions.yml` checks all thirteen companion repos' `uq/` folders against this uQ each
week and when the page loader changes. For a manual check in a real session, see
[`notes/settings-slice-e-live-check.md`](notes/settings-slice-e-live-check.md).
