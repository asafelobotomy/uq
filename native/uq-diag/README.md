# uq-diag

Makes a native app (Rust or Qt 6 C++) log and crash the way uQ does, so its problems show up in
`uqctl log`, `uqctl explain` and Settings › Problems beside uQ's own. One copy of the code instead
of one per app. The whole scheme is in [`docs/diagnostics.md`](../../docs/diagnostics.md).

Version: **1.0.1** (the first line of each file says `uq-diag 1.0.1`). 1.0.1 builds on Rust
edition 2021 as well as 2024; nothing else changed.

| File | For | Needs |
|---|---|---|
| `uq_diag.rs` | Rust apps | `tracing`, `tracing-subscriber` (`env-filter`), `serde_json` |
| `uqdiag.h`, `uqdiag.cpp` | Qt 6 C++ apps | Qt Core |
| `check-uq-diag.sh` | every app's CI | bash, sha256sum |

## What you get

- **One log format.** Under systemd each line starts with its syslog priority (`<4>reelo: …`), so
  journald files it at the right level; in a terminal it reads `WARN reelo: …`. No colours, no
  duplicate timestamps (the journal keeps its own).
- **Crash records.** A Rust panic, or a C++ crash (SIGSEGV, SIGABRT, SIGBUS, SIGFPE, SIGILL),
  leaves `crash.json` (and for C++ `backtrace.txt`) in
  `$XDG_STATE_HOME/uq/crashes/<program>-<time>-<pid>/`. uqd notices the new folder, writes a
  report beside it, says so in a notification and lists it on Settings › Problems with
  **Copy report**. The program still dies of its own signal, so systemd and coredumps see it.

## Codes

Start every warning and error with your app's code in brackets, an upper-case prefix of your
own, a dash and three digits:

```rust
tracing::warn!("[BINDLE-003] couldn't refresh the catalogue: {e}");
```
```qml
console.warn("reelo[REELO-010] subtitle file unreadable: " + path)
```

and install the list as `/usr/share/uq/diag/<app>.toml`:

```toml
app = "Bindle"                       # the name people know
units = ["bindle-update.service"]    # systemd user units whose journal `uqctl log` reads

[[code]]
id = "BINDLE-003"
severity = "warn"                    # error, warn or info
title = "The catalogue couldn't be refreshed"
means = "Bindle couldn't download the app list, so it shows the one from last time."
fix = "Check the network connection, then press Refresh."
```

`means` and `fix` are shown on Settings › Problems as they are, so write them in plain words.
Keep the file in your repo as `uq/diag/<app>.toml`: `uqctl integrations check uq/` checks it
(it parses, names the app, one prefix of your own, no code twice, each with a title, a meaning
and a fix).
Never reuse a number for something else: set `retired = true` on a code nothing logs any more.

## Rust

Copy `uq_diag.rs` to `src/uq_diag.rs`, add the three crates, and declare it so your own rustfmt
settings leave it alone:

```rust
#[rustfmt::skip]
mod uq_diag;

fn main() {
    uq_diag::init("bindle", "info");   // RUST_LOG still wins
    // …
}
```

`init_with(program, filter)` takes an `EnvFilter` you built; `install_panic_hook(program)` adds
only the crash record (for a command-line tool whose stderr is for people).

## C++ (Qt 6)

Copy `uqdiag.h` and `uqdiag.cpp` into your sources:

```cpp
#include "uqdiag.h"

int main(int argc, char **argv)
{
    uqdiag::init("reelo", APP_VERSION);   // before QGuiApplication
    QGuiApplication app(argc, argv);
    // …
}
```

## Keeping copies in step

Don't edit the vendored files; to update, copy the new release over them and bump the pin. In CI:

```sh
scripts/check-uq-diag.sh 1.0.1 src/uq_diag.rs
```

Changing these files here: bump the version on the first line of all three, run `./gen-check.sh`
and copy the new script into the apps along with the files. uQ's CI runs the tests in `test/` (a
real crash included) and fails when `check-uq-diag.sh` is out of date.

## Who uses it

| App | Copy |
|---|---|
| uQ's uq-core and uqlip | Rust (this file, by path) |
| Bindle (`BINDLE-`) | Rust, `crates/bindle-core/src/uq_diag.rs` |
| noti-fi (`NOTIFI-`) | Rust, `crates/notify-diag` |
| uQview (`UQVIEW-`) | Rust, `crates/uqview/src/uq_diag.rs` |
| reelo (`REELO-`) | C++, `src/uqdiag.*` |
| uQbit (`UQBIT-`) | C++, `src/uqdiag.*` |
