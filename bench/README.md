# Benchmarks

`bench/live.py` runs the real shell (`qs -p shell`) and `uqd` on a headless Hyprland in a private
session, and writes one JSON file of metrics (all "lower is better").

| Metric | How |
|---|---|
| `shell_startup_ms`, `uqd_startup_ms` | process start until the bar layer exists / `org.uq.Desktop1` is on the bus; median of 5 |
| `*_idle_rss`, `*_threads` | `/proc/PID/status` after 8 s of settling |
| `*_idle_cpu_pct`, `*_idle_children_cpu_pct` | CPU ticks of the process, and of children it waited for, over a 25 s idle window placed inside one wall-clock minute (the bar clock repaints at the minute change) |
| `*_idle_wakeups_per_s` | voluntary + involuntary context switches over all threads, per second |
| `*_idle_spawns_per_min`, `shell_startup_spawns` | `execve` calls counted with `strace --seccomp-bpf` |
| `shell_idle_frames_per_min` | frames Qt renders while nothing happens (Qt's `QSG_RENDER_TIMING`) |
| `popup_open/close_lookseedo_ms` | `qs ipc call uq open/close lookseedo` until the layer maps/unmaps (includes the constant cost of the `qs ipc` client) |
| `shell_rss_after_popups`, `shell_rss_growth_popups` | memory after 10 open/close cycles (after 2 warm-up cycles); the growth is noisy, so it only fails when it triples and passes 32 MiB |
| `uqlip_idle_*` | the clipboard daemon, which also runs in the session so the shell's watcher behaves as on a desktop |
| `frame_p50_ms`, `frame_p95_ms` | render time of each frame (the "frame rendered in" lines of `QSG_RENDER_TIMING`) while popups open and close |

Rendering is software (llvmpipe on vkms), so frame times and latencies only mean something against
another run on the same machine. CPU, memory, wakeups, spawns and idle frames are not renderer
dependent.

## In CI

`.github/workflows/bench.yml` builds a pull request's base and head, measures both in two
alternating rounds on one runner (`bench/in-container.sh`), and `bench/compare.py` compares the
medians against `bench/thresholds.json`. The table lands in the job summary; the check fails when a
metric grows past its `fail` limit. A push to `main` uploads its numbers as the `bench-results`
artifact.

## Locally

Needs Hyprland, quickshell, a DRM device and a release build of `uqctl` and `uqd`:

```sh
cargo build --release -p uqctl -p uqd -p uqlip
python3 bench/live.py --src . --out bench-live.json     # --quick for a short run
```

On a machine with a GPU, `BENCH_HW=1 python3 bench/live.py --diag --src . --out /tmp/hw.json` renders on
the GPU instead of llvmpipe, and `--diag` prints every popup's frames in full. `popup_first_sync_ms`
is how long the GUI thread waits for a popup's first frame (surface and swapchain creation).

Rust micro-benchmarks for `uq-core` are not in yet; the shell and daemon figures above are the
user-visible ones.
