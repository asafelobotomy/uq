import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { applyTokens, clearTokens, parseTokens, PROPERTIES, VERSION } from "../uq-tokens.js";
import { fakeRoot } from "./fake-root.js";

const golden = (name) => readFileSync(fileURLToPath(new URL(
    `../../../crates/uq-core/tests/golden/${name}/tokens.json`, import.meta.url)), "utf8");

test("maps the real graphite-dark tokens.json", () => {
    const r = fakeRoot();
    const n = applyTokens(parseTokens(golden("graphite-dark")), r);
    assert.equal(n, PROPERTIES.length);
    const v = (k) => r.style.getPropertyValue(k);
    assert.equal(v("--uq-bg"), "#1f1f22");
    assert.equal(v("--uq-surface"), "#2b2b2e");
    assert.equal(v("--uq-surface-alt"), "#353538");
    assert.equal(v("--uq-overlay"), "#464649");
    assert.equal(v("--uq-text"), "#e4e4e7");
    assert.equal(v("--uq-text-muted"), "#a9a9ac");
    assert.equal(v("--uq-accent"), "#8ab4f8");
    assert.equal(v("--uq-accent-text"), "#1f1f22");
    assert.equal(v("--uq-border"), "#5a5a5d");
    assert.equal(v("--uq-success"), "#81c995");
    assert.equal(v("--uq-warning"), "#fdd663");
    assert.equal(v("--uq-error"), "#f28b82");
    assert.equal(v("--uq-radius"), "10px");
    assert.equal(v("--uq-gap"), "5px");
    assert.equal(v("--uq-gap-in"), "5px");
    assert.equal(v("--uq-gap-out"), "12px");
    assert.equal(v("--uq-border-width"), "2px");
    assert.equal(v("--uq-font-size"), "11px");
    assert.equal(v("--uq-font-ui"), '"Inter"');
    assert.equal(v("--uq-font-mono"), '"JetBrains Mono"');
    assert.equal(r.getAttribute("data-uq-variant"), "dark");
});

test("every golden theme maps fully", () => {
    for (const t of ["gruvbox-light", "dracula-dark", "velvet-light", "forest-light"]) {
        const r = fakeRoot();
        assert.equal(applyTokens(parseTokens(golden(t)), r), PROPERTIES.length, t);
        assert.equal(r.getAttribute("data-uq-variant"), t.endsWith("light") ? "light" : "dark");
    }
});

test("unknown keys are ignored", () => {
    const r = fakeRoot();
    applyTokens({ colors: { bg: "#000000", evil: "#ffffff", __proto__: { x: 1 } },
        ui: { radius: 4, extra: 9 }, terminal: { ansi: ["#000000"] }, icons: { theme: "x" } }, r);
    assert.deepEqual([...r.props.keys()].sort(), ["--uq-bg", "--uq-radius"]);
    assert.ok([...r.props.keys()].every((k) => PROPERTIES.includes(k)));
});

test("8-digit colours and light variant", () => {
    const r = fakeRoot();
    applyTokens({ variant: "light", colors: { accent: "#11223344" } }, r);
    assert.equal(r.style.getPropertyValue("--uq-accent"), "#11223344");
    assert.equal(r.getAttribute("data-uq-variant"), "light");
});

test("clearTokens removes everything", () => {
    const r = fakeRoot();
    applyTokens(parseTokens(golden("graphite-dark")), r);
    r.style.setProperty("--app-own", "1");
    clearTokens(r);
    assert.deepEqual([...r.props.keys()], ["--app-own"]);
    assert.equal(r.getAttribute("data-uq-variant"), null);
});

test("version and property list", () => {
    assert.equal(VERSION, "1.0.0");
    assert.ok(PROPERTIES.includes("--uq-font-ui") && PROPERTIES.includes("--uq-gap"));
    assert.ok(Object.isFrozen(PROPERTIES));
});

test("no root (no DOM) is a no-op", () => {
    assert.equal(applyTokens({ colors: { bg: "#000000" } }), 0);
    assert.doesNotThrow(() => clearTokens());
});
