import { test } from "node:test";
import assert from "node:assert/strict";
import { watchTokens } from "../uq-tokens.js";
import { fakeRoot } from "./fake-root.js";

const full = (bg, variant = "dark") => JSON.stringify({ variant, colors: { bg, text: "#eeeeee" },
    ui: { radius: 10, gap_in: 5, gap_out: 12, border_width: 2 }, fonts: { ui: "Inter", mono: "Mono", size: 11 } });

function source(initial) {
    let text = initial, cb = null, unsubscribed = 0;
    return {
        set(t) { text = t; cb && cb(); },
        read: () => text,
        subscribe(f) { cb = f; return () => { unsubscribed++; cb = null; }; },
        get unsubscribed() { return unsubscribed; }
    };
}

test("applies on start and on every change", async () => {
    const r = fakeRoot(), s = source(full("#111111"));
    const w = watchTokens({ read: s.read, subscribe: s.subscribe, root: r });
    await w.ready;
    assert.equal(r.style.getPropertyValue("--uq-bg"), "#111111");
    s.set(full("#222222", "light"));
    assert.equal(r.style.getPropertyValue("--uq-bg"), "#222222");
    assert.equal(r.getAttribute("data-uq-variant"), "light");
    w.stop();
    assert.equal(s.unsubscribed, 1);
});

test("falls back when read() returns null, bad JSON, or throws, then recovers", async () => {
    const r = fakeRoot(), s = source(full("#111111"));
    watchTokens({ read: s.read, subscribe: s.subscribe, root: r });
    assert.equal(r.props.size, 10);   // 2 colours, 6 lengths, 2 fonts: only what was given
    s.set(null);
    assert.equal(r.props.size, 0);
    assert.equal(r.getAttribute("data-uq-variant"), null);
    s.set(full("#333333"));
    assert.equal(r.style.getPropertyValue("--uq-bg"), "#333333");
    s.set("{\"colors\": {");
    assert.equal(r.props.size, 0);
    const w2 = watchTokens({ read: () => { throw new Error("EIO"); }, root: r });
    await w2.ready;
    assert.equal(r.props.size, 0);
});

test("repeated changes remove stale properties", () => {
    const r = fakeRoot(), s = source(full("#111111"));
    watchTokens({ read: s.read, subscribe: s.subscribe, root: r });
    assert.equal(r.style.getPropertyValue("--uq-font-ui"), '"Inter"');
    s.set(JSON.stringify({ variant: "light", colors: { accent: "#abcdef" } }));
    assert.deepEqual([...r.props.keys()], ["--uq-accent"]);
    assert.equal(r.getAttribute("data-uq-variant"), "light");
    s.set(JSON.stringify({ colors: { bg: "#000000" } }));
    assert.deepEqual([...r.props.keys()], ["--uq-bg"]);
    assert.equal(r.getAttribute("data-uq-variant"), null);
});

test("async read and async subscribe (Tauri style)", async () => {
    const r = fakeRoot();
    let text = full("#111111"), cb = null, unlistened = false;
    const w = watchTokens({
        read: () => Promise.resolve(text),
        subscribe: (f) => { cb = f; return Promise.resolve(() => { unlistened = true; }); },
        root: r
    });
    await w.ready;
    assert.equal(r.style.getPropertyValue("--uq-bg"), "#111111");
    text = full("#444444");
    cb();
    await w.refresh();
    assert.equal(r.style.getPropertyValue("--uq-bg"), "#444444");
    const rejecting = watchTokens({ read: () => Promise.reject(new Error("gone")), root: r });
    await rejecting.ready;
    assert.equal(r.props.size, 0);
    await new Promise((res) => setTimeout(res, 0));
    w.stop();
    assert.equal(unlistened, true);
});

test("a slow older read does not overwrite a newer one", async () => {
    const r = fakeRoot();
    const pending = [];
    const w = watchTokens({ read: () => new Promise((res) => pending.push(res)), root: r });
    const second = w.refresh();
    pending[1](full("#222222"));
    await second;
    pending[0](full("#111111"));
    await w.ready;
    assert.equal(r.style.getPropertyValue("--uq-bg"), "#222222");
});

test("nothing is applied after stop, and bad arguments never throw", async () => {
    const r = fakeRoot(), s = source(full("#111111"));
    const w = watchTokens({ read: s.read, subscribe: s.subscribe, root: r });
    w.stop();
    r.props.clear();
    await w.refresh();
    assert.equal(r.props.size, 0);
    assert.doesNotThrow(() => watchTokens());
    assert.doesNotThrow(() => watchTokens({ read: 5, subscribe: () => { throw new Error("x"); }, root: r }));
});
