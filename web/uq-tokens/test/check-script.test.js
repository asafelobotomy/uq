import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(new URL("../check-uq-tokens.sh", import.meta.url));
const module = fileURLToPath(new URL("../uq-tokens.js", import.meta.url));
const run = (...args) => spawnSync("bash", [script, ...args], { encoding: "utf8" });

test("matches the pinned version of the real module", () => {
    const r = run("1.0.0", module);
    assert.equal(r.status, 0, r.stderr);
});

test("fails on a mismatch, a missing header, a missing file", () => {
    assert.equal(run("1.0.1", module).status, 1);
    const dir = mkdtempSync(join(tmpdir(), "uq-tokens-"));
    const old = join(dir, "old.js"), none = join(dir, "none.js");
    writeFileSync(old, "// uq-tokens 0.9.0\nexport {};\n");
    writeFileSync(none, "export {};\n");
    const r = run("1.0.0", old);
    assert.equal(r.status, 1);
    assert.match(r.stderr, /0\.9\.0, pinned 1\.0\.0/);
    assert.equal(run("0.9.0", old).status, 0);
    assert.equal(run("1.0.0", none).status, 1);
    assert.equal(run("1.0.0", join(dir, "absent.js")).status, 1);
});

test("bad usage exits 2", () => {
    assert.equal(run().status, 2);
    assert.equal(run("1.0", module).status, 2);
    assert.equal(run("1.0.0").status, 2);
});
