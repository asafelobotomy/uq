# Launcher provider template

A small Rust D-Bus service that adds results to uQ's Spotlight. It is a standalone example crate,
not part of uQ's workspace: copy the folder into your repository.

```
src/main.rs                          the provider: Search and Activate, exits after 60 s idle
uq/launcher/example.provider         → /usr/share/uq/launcher/
uq/dbus/org.example.Provider.service → /usr/share/dbus-1/services/
```

1. Rename `example-provider` (the binary) and `org.example.Provider` (the bus name) in every
   file, and rename the `.service` file to match the bus name.
2. Write your search in `Provider::search`. uQ waits at most 300 ms for an answer and shows at
   most 20 rows; return quickly, and an empty list when you have nothing.
3. Handle the actions you offered in `Provider::activate`. Never run anything taken from the
   query or the id through a shell.
4. Package the binary in `/usr/bin` and install the two files above. Run
   `uqctl integrations check uq/` in your CI.

How it starts: with `Activatable=true` and the service file, uQ asks the session bus to start the
provider when Spotlight or the app menu opens (at most once every 30 s), never while you type.
Queries go only to a running provider, so the first keystrokes after a cold start may miss its
results. The provider exits after 60 s without a call, so it only runs while it's useful.

Try it by hand: `cargo run`, then
`busctl --user call org.example.Provider /org/uq/Launcher1 org.uq.Launcher1.Provider Search s ap`.

The full reference is `docs/integrating.md` in the uQ repository.
