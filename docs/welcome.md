# Welcome, and the bindle hand-off

**Welcome** is the short tour uQ shows in Settings the first time someone logs in. It walks
through the choices that matter and applies each one the moment it is made. This page covers
when it opens, how to open it again, and the contract installers such as bindle follow so their
own first-login window and Welcome never appear at once.

## What it shows

Each step can be skipped, and Back goes to the step before.

1. **Hello.** What uQ is, and three keys: **Super + Space** (search), **tap Super** (app menu),
   **Super + A** (quick settings). If the session's first-login setup reported a problem, a card
   says so and names `~/.local/state/uq/session.log`.
2. **Look.** Theme, light or dark, and switching between them on a schedule.
3. **Wallpaper.** The Wallpaper page.
4. **Layouts.** How windows are arranged by default (Dwindle, Master or Scrolling).
5. **Displays.** Only with more than one screen: the Displays page, with its 15-second "keep these
   settings?" check.
6. **Apps.** Which apps made for uQ are installed. For each missing one, a line on what it adds
   and its package name to copy; **Install with bindle** starts bindle when `/usr/bin/bindle`
   exists.
7. **Fingerprint.** Only with a fingerprint reader and no finger saved yet: saves the right index
   finger through fprintd, so the lock screen can use it.
8. **Done.** Where Settings is, the keys worth knowing, and how to see Welcome again.

The Look, Wallpaper, Layouts and Displays steps use the same uqd calls as the full Settings pages,
so their changes show in History and can be undone there.

## When it opens

uqd keeps `[welcome]` in `~/.local/state/uq/state.toml`: `done` and `version`. Welcome is *due*
when it was never finished, or when a newer uQ added steps (then only the new steps show, once).
Whether Welcome ran is not a setting: it never appears in History, and a restore point never brings
it back.

When the shell starts, it asks the bridge whether Welcome is due. If it is, the shell waits 20
seconds, asks again, and then, unless an installer is holding Welcome back (below), runs
`uq-settings --welcome` once. The wait gives an installer started from the session's autostart
time to say it is running.

Welcome counts as done as soon as it shows. Closing the window at any step is fine; nothing asks
again, and a shell that restarts doesn't open it a second time.

## Opening it again

- **Settings → About → Show Welcome** shows every step from the start.
- `uqctl welcome` opens Welcome if it is due. If it was already finished, it prints a note and
  exits 0 without opening anything.
- `uqctl welcome --reset` makes Welcome due again from its first step, then opens it.

## The hand-off contract (for bindle and other installers)

An installer that runs its own first-login window inside a uQ session (bindle's app picker does)
holds Welcome back while that window is open, then hands over to it:

1. **Defer.** When the window opens, create the file

   ```
   $XDG_RUNTIME_DIR/uq/welcome.defer
   ```

   (create the `uq` folder if needed; the contents don't matter). While the file exists and was
   **modified less than 30 minutes ago**, the shell doesn't start Welcome. Writing the file again
   refreshes its age, so a long wizard can touch it now and then.
2. **Hand off.** When the window closes, remove the file, then run

   ```
   uqctl welcome
   ```

   It opens Welcome at step 1, whose Apps step then shows what was just installed. Don't pass
   `--reset`: if the user already finished Welcome, `uqctl welcome` rightly does nothing.

Rules uQ keeps, so an installer can rely on them:

- A defer file **30 minutes old or older is ignored**, so a crashed installer can't hide Welcome for
  good; it then opens at the next login. A file dated more than a minute in the future is ignored
  too.
- uQ only reads the file's modification time. It never writes or removes it.
- Without `XDG_RUNTIME_DIR`, nothing is deferred.
- The shell decides once per login: when it starts, and again 20 seconds later if Welcome is due.
  Create the file within those 20 seconds of the session starting (an XDG autostart entry is
  early enough). A file created after Welcome opened has no effect on it.
- `uqctl welcome` needs uqd running (it exits 1 with "uqd is not running" otherwise) and opens
  Welcome through the Settings app's single instance, so it never opens a second window.

bindle implements its side in `crates/bindle-cli/src/uq_handoff.rs`.
