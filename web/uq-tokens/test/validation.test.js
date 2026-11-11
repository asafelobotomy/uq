import { test } from "node:test";
import assert from "node:assert/strict";
import { applyTokens, parseTokens, validColor, validLength, validFont } from "../uq-tokens.js";
import { fakeRoot } from "./fake-root.js";

test("colours: only #rrggbb and #rrggbbaa", () => {
    for (const ok of ["#000000", "#AbCdEf", "#00000000"]) assert.equal(validColor(ok), ok);
    for (const bad of ["red", "#fff", "#fff;", "#ffff", "#1234567", "#gggggg", "red;}body{x",
        "#000000;}body{x", "#000000 ", " #000000", "#000000\n", "rgb(0,0,0)", "url(x)", "",
        null, undefined, 0, ["#000000"], { toString: () => "#000000" }])
        assert.equal(validColor(bad), null, String(bad));
});

test("lengths: finite non-negative numbers, emitted as px", () => {
    assert.equal(validLength(0), "0px");
    assert.equal(validLength(10), "10px");
    assert.equal(validLength(1.5), "1.5px");
    for (const bad of ["10", "10px", "10px;}x{", -1, NaN, Infinity, 1e9, null, true, [4], {}])
        assert.equal(validLength(bad), null, String(bad));
});

test("fonts: plain family names only, quoted on output", () => {
    assert.equal(validFont("Inter"), '"Inter"');
    assert.equal(validFont("JetBrains Mono"), '"JetBrains Mono"');
    assert.equal(validFont("Noto Sans CJK JP"), '"Noto Sans CJK JP"');
    for (const bad of ["", "   ", "x".repeat(101), "Inter;", "Inter}body{color:red", "a{b", "Inter\"",
        "Inter'", "Inter\\", "Inter\nx", "Inter\rx", "Inter\tx", "Inter\u0000", "url(https://x/)",
        "x URL ( y", "a /* b", 7, null, ["Inter"]])
        assert.equal(validFont(bad), null, JSON.stringify(bad));
});

test("CSS injection attempts never reach the root", () => {
    const r = fakeRoot();
    applyTokens({
        variant: "dark;}body{x",
        colors: { bg: "red;}body{x", text: "#fff;", accent: "url(javascript:alert(1))",
            border: "#000000;background:url(x)" },
        ui: { radius: "10px;}body{x", gap_in: "5", gap_out: { valueOf: () => 3 } },
        fonts: { ui: "Inter}body{color:red", mono: "x\");background:url(evil", size: "11" }
    }, r);
    assert.equal(r.props.size, 0);
    assert.equal(r.getAttribute("data-uq-variant"), null);
});

test("one bad value does not stop the rest", () => {
    const r = fakeRoot();
    applyTokens({ colors: { bg: "red", text: "#ffffff" }, ui: { radius: -3, gap_in: 4 } }, r);
    assert.deepEqual([...r.props.keys()].sort(), ["--uq-gap", "--uq-gap-in", "--uq-text"]);
});

test("parseTokens never throws and returns null on bad input", () => {
    for (const bad of [null, undefined, 42, "", "{", "not json", "[]", "null", "\"x\"", "1", {}, "{\"a\":"])
        assert.equal(parseTokens(bad), null, String(bad));
    assert.deepEqual(parseTokens("{\"variant\":\"dark\"}"), { variant: "dark" });
});

test("applyTokens with non-objects clears instead of throwing", () => {
    const r = fakeRoot();
    applyTokens({ colors: { bg: "#000000" }, variant: "dark" }, r);
    for (const bad of [null, undefined, "{}", 3, [], () => 1]) {
        assert.equal(applyTokens(bad, r), 0);
        assert.equal(r.props.size, 0);
        assert.equal(r.getAttribute("data-uq-variant"), null);
    }
});

test("a throwing root does not make applyTokens throw", () => {
    const r = { style: { setProperty() { throw new Error("x"); }, removeProperty() { throw new Error("y"); } },
        setAttribute() { throw new Error("z"); }, removeAttribute() { throw new Error("w"); } };
    assert.doesNotThrow(() => applyTokens({ colors: { bg: "#000000" } }, r));
    assert.doesNotThrow(() => applyTokens(null, r));
});

test("the module source avoids innerHTML, eval and Function", async () => {
    const { readFileSync } = await import("node:fs");
    const src = readFileSync(new URL("../uq-tokens.js", import.meta.url), "utf8");
    const code = src.split("\n").filter((l) => !l.trim().startsWith("//")).join("\n");
    assert.doesNotMatch(code, /innerHTML|outerHTML|insertAdjacentHTML|\beval\s*\(|new Function|cssText|document\.write/);
    assert.match(src.split("\n")[0], /^\/\/ uq-tokens \d+\.\d+\.\d+$/);
});
