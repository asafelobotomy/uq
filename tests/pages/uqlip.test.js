import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

// logic.js is a QML `.pragma library`; evaluate it as a plain script.
const src = readFileSync(new URL("../../uq/pages/uqlip/logic.js", import.meta.url), "utf8")
    .replace(/^\.pragma library/m, "");
const L = new Function(`${src}; return { problemOf, describe, apps, validApp, textKb };`)();

test("problemOf ignores success and cancel", () => {
    assert.equal(L.problemOf({ code: 0 }), "");
    assert.equal(L.problemOf({ error: "x", cancelled: true }), "");
    assert.equal(L.problemOf({ error: "boom" }), "boom");
    assert.equal(L.problemOf(null), "");
});

test("describe", () => {
    assert.match(L.describe(null), /isn't running/);
    assert.equal(L.describe({ count: 1, pinned: 0, paused: false, enabled: true }), "Recording. 1 item kept.");
    assert.equal(L.describe({ count: 5, pinned: 2, paused: true, enabled: true }), "Paused. 5 items kept, 2 pinned.");
    assert.match(L.describe({ count: 0, enabled: false }), /^Off\./);
});

test("apps are sorted and unique ignoring case", () => {
    const cfg = { privacy: { ignore_apps: ["keepassxc", "Bitwarden", "KeePassXC"] } };
    assert.deepEqual(L.apps(cfg), ["Bitwarden", "keepassxc"]);
    assert.deepEqual(L.apps(null), []);
});

test("validApp follows page.toml", () => {
    assert.ok(L.validApp("org.keepassxc.KeePassXC"));
    assert.ok(L.validApp("Proton Pass"));
    assert.ok(!L.validApp("-rf"));
    assert.ok(!L.validApp(""));
    assert.ok(!L.validApp("a".repeat(65)));
});

test("textKb", () => {
    assert.equal(L.textKb({ privacy: { max_text_bytes: 2048 } }), 2);
    assert.equal(L.textKb(null), 1024);
});
