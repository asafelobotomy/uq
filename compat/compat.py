#!/usr/bin/env python3
"""The uQ family's versions: checks compat/family.toml and the repos against it.

  compat.py check [--repos DIR]   the manifest; with DIR (checkouts named like each repo), each
                                  PKGBUILD's uq-compat line and what the repo's files say it uses
  compat.py line PACKAGE          the uq-compat line for that package's PKGBUILD(s)
  compat.py sync DIR              write the uq-compat line into each PKGBUILD under DIR
  compat.py releases DIR          the latest releases checked out in DIR: each one's needs are met
                                  by its providers' latest releases
  compat.py aur [DIR]             the AUR's versions: every package's conflicts can be met
                                  there; with DIR (latest releases), which are behind
                                  (needs the network)

Python 3.11+, nothing else. Exit 1 on a problem, each named on stderr.
"""

from __future__ import annotations

import json
import pathlib
import re
import sys
import tomllib
import urllib.request

HERE = pathlib.Path(__file__).resolve().parent
MANIFEST = HERE / "family.toml"
MARK = "# uq-compat: from uQ's compat/family.toml"
AUR_INFO = "https://aur.archlinux.org/rpc/v5/info"
FUNC = re.compile(r"^(\s*)(prepare|pkgver|build|check|package)[\w-]*\(\)\s*\{")
VERSION = re.compile(r"^\d+(\.\d+)*$")


def vkey(v: str) -> tuple[int, ...]:
    """A plain dotted release (`0.3.41`, a tag without its `v`), for ordering."""
    return tuple(int(p) for p in v.split("-")[0].split("."))


def load(path: pathlib.Path = MANIFEST) -> dict:
    with open(path, "rb") as f:
        return tomllib.load(f)


def problems(m: dict) -> list[str]:
    """What is wrong with the manifest itself."""
    out = []
    if m.get("schema") != 1:
        out.append("schema must be 1")
    contracts, packages = m.get("contract", {}), m.get("package", {})
    for name, c in contracts.items():
        if c.get("provider") not in packages:
            out.append(f"contract {name}: provider {c.get('provider')!r} is not a package")
        levels = c.get("levels", {})
        if not levels:
            out.append(f"contract {name}: no levels")
        for lv, v in {**levels, **c.get("dropped", {})}.items():
            if not lv.isdigit() or not VERSION.match(str(v)):
                out.append(f"contract {name}: level {lv} = {v!r} is not a level and a release")
        for lv, v in c.get("dropped", {}).items():
            if lv in levels and vkey(v) <= vkey(levels[lv]):
                out.append(f"contract {name}: level {lv} is dropped before it starts")
    for name, p in packages.items():
        for key in ("repo", "aur", "pkgbuild"):
            if not p.get(key):
                out.append(f"package {name}: no {key}")
        for k, lv in p.get("needs", {}).items():
            c = contracts.get(k)
            if c is None:
                out.append(f"package {name}: needs unknown contract {k}")
            elif str(lv) not in c.get("levels", {}):
                out.append(f"package {name}: {k} has no level {lv}")
            elif c["provider"] == name:
                out.append(f"package {name}: needs its own contract {k}")
        for prov, v in p.get("min", {}).items():
            if prov not in packages or not VERSION.match(str(v)):
                out.append(f"package {name}: min {prov} = {v!r}")
        for k, v in p.get("since", {}).items():
            if k not in contracts or not VERSION.match(str(v)):
                out.append(f"package {name}: since {k} = {v!r}")
    return out


def bounds(m: dict, name: str) -> list[str]:
    """The pacman relations for one package: its providers' floors (and ceilings where a level
    it needs was dropped), and, as a provider, the users a dropped level would break."""
    contracts, packages = m["contract"], m["package"]
    p = packages[name]
    floors: dict[str, str] = {}
    ceilings: dict[str, str] = {}
    for k, lv in p.get("needs", {}).items():
        c = contracts[k]
        prov, start = c["provider"], c["levels"][str(lv)]
        floor = max([start, p.get("min", {}).get(prov, start)], key=vkey)
        if prov not in floors or vkey(floor) > vkey(floors[prov]):
            floors[prov] = floor
        end = c.get("dropped", {}).get(str(lv))
        if end and (prov not in ceilings or vkey(end) < vkey(ceilings[prov])):
            ceilings[prov] = end
    users: dict[str, str] = {}
    for k, c in contracts.items():
        if c["provider"] != name or not c.get("dropped"):
            continue
        for user, q in packages.items():
            since = q.get("since", {}).get(k)
            if since and k in q.get("needs", {}):
                users[user] = max([since, users.get(user, since)], key=vkey)
    out = []
    for prov in sorted(floors):
        for aur in packages[prov]["aur"]:
            out.append(f"{aur}<{floors[prov]}")
            if prov in ceilings:
                out.append(f"{aur}>={ceilings[prov]}")
    for user in sorted(users):
        out += [f"{aur}<{users[user]}" for aur in packages[user]["aur"]]
    return out


def line(m: dict, name: str) -> str:
    rel = " ".join(f"'{b}'" for b in bounds(m, name))
    return f"conflicts+=({rel})  {MARK}"


def repo_dir(root: pathlib.Path, p: dict) -> pathlib.Path:
    return root / p["repo"].split("/")[1]


def sync_text(text: str, want: str) -> str:
    """The PKGBUILD (or workflow writing one) with `want` as its uq-compat line: in place of the
    old one, else before its first function, indented like it."""
    lines = text.split("\n")
    for i, l in enumerate(lines):
        if MARK in l:
            lines[i] = l[: len(l) - len(l.lstrip())] + want
            return "\n".join(lines)
    for i, l in enumerate(lines):
        f = FUNC.match(l)
        if f:
            return "\n".join(lines[:i] + [f.group(1) + want, f.group(1).rstrip()] + lines[i:])
    raise ValueError("no function to put the uq-compat line before")


def uses(repo: pathlib.Path) -> dict[str, int]:
    """What a checkout says it uses: the newest uq_api of its integration files, and the major
    of each uq-theme and uq-diag kit it vendors."""
    found: dict[str, int] = {}
    for f in sorted((repo / "uq").glob("**/*.toml")) if (repo / "uq").is_dir() else []:
        if "diag" in f.parts or f.name == "native.toml":
            continue
        try:
            api = tomllib.loads(f.read_text()).get("uq_api", 1)
        except (tomllib.TOMLDecodeError, UnicodeDecodeError):
            continue
        if isinstance(api, int):
            found["uq-api"] = max(found.get("uq-api", 0), api)
    native = repo / "uq" / "native.toml"
    if native.is_file():
        kits = tomllib.loads(native.read_text())
        for kit, contract in (("uq-theme", "uq-theme"), ("uq-diag", "uq-diag")):
            if kit in kits:
                found[contract] = int(str(kits[kit]["version"]).split(".")[0])
    return found


def check_repos(m: dict, root: pathlib.Path) -> list[str]:
    out = []
    for name, p in m["package"].items():
        repo = repo_dir(root, p)
        if not repo.is_dir():
            out.append(f"{name}: no checkout at {repo}")
            continue
        want = line(m, name)
        for rel in p["pkgbuild"]:
            f = repo / rel
            got = [l.strip() for l in f.read_text().splitlines() if MARK in l] if f.is_file() else None
            if got is None:
                out.append(f"{name}: {rel} is missing")
            elif got != [want]:
                out.append(f"{name}: {rel} should say\n    {want}\n  (compat.py sync)")
        needs = p.get("needs", {})
        for k, lv in uses(repo).items():
            if m["contract"][k]["provider"] != name and needs.get(k) != lv:
                out.append(f"{name}: its files use {k} {lv}, family.toml says {needs.get(k)}")
    return out


def release_of(repo: pathlib.Path) -> str | None:
    """A checkout's release: VERSION, else the first `version =` of Cargo.toml or package.json."""
    v = repo / "VERSION"
    if v.is_file():
        return v.read_text().strip()
    for f, pat in (("Cargo.toml", r'^version\s*=\s*"([^"]+)"'), ("package.json", r'"version"\s*:\s*"([^"]+)"')):
        if (repo / f).is_file():
            g = re.search(pat, (repo / f).read_text(), re.M)
            if g:
                return g.group(1)
    return None


def unmet(m: dict, released: dict[str, str]) -> list[str]:
    """Needs the released versions (`package -> version`) don't meet."""
    out = []
    for name, p in m["package"].items():
        for k, lv in p.get("needs", {}).items():
            c = m["contract"][k]
            have = released.get(c["provider"])
            if have is None:
                continue
            start = max([c["levels"][str(lv)], p.get("min", {}).get(c["provider"], "0")], key=vkey)
            end = c.get("dropped", {}).get(str(lv))
            if vkey(have) < vkey(start):
                out.append(f"{name} needs {k} {lv} from {c['provider']} {start}; its latest release is {have}")
            if end and vkey(have) >= vkey(end):
                out.append(f"{c['provider']} {have} dropped {k} {lv}, which {name}'s latest release needs")
    return out


def aur_info(names: list[str]) -> dict[str, dict]:
    url = AUR_INFO + "?" + "&".join(f"arg[]={n}" for n in names)
    with urllib.request.urlopen(url, timeout=30) as r:
        return {x["Name"]: x for x in json.load(r)["results"]}


def aur_problems(m: dict, info: dict[str, dict]) -> tuple[list[str], dict[str, str]]:
    """Clashes among the AUR's own versions (what its packages declare, not what family.toml says
    now: an older version on the AUR declares older needs), and each package's AUR version."""
    from_aur = {n: x["Version"] for n, x in info.items()}
    out = []
    for name, p in m["package"].items():
        for aur in p["aur"]:
            if aur not in info:
                out.append(f"{aur} is not on the AUR")
                continue
            for rel in info[aur].get("Conflicts") or []:
                g = re.match(r"^([^<>=]+)(<|>=)(.+)$", rel)
                if not g or g.group(1) not in from_aur:
                    continue
                have, op, v = vkey(from_aur[g.group(1)]), g.group(2), vkey(g.group(3))
                if (op == "<" and have < v) or (op == ">=" and have >= v):
                    out.append(f"{aur} {from_aur[aur]} conflicts with {rel}, and the AUR has {g.group(1)} {from_aur[g.group(1)]}")
    return out, from_aur


def main(argv: list[str]) -> int:
    m = load()
    errs = problems(m)
    cmd = argv[1] if len(argv) > 1 else "check"
    if errs or cmd == "check" and len(argv) < 3:
        for e in errs:
            print(f"family.toml: {e}", file=sys.stderr)
        if not errs:
            print(f"compat: {len(m['contract'])} contracts, {len(m['package'])} packages")
        return 1 if errs else 0
    if cmd == "check" and argv[2] == "--repos":
        errs = check_repos(m, pathlib.Path(argv[3]))
    elif cmd == "line":
        print(line(m, argv[2]))
    elif cmd == "sync":
        for name, p in m["package"].items():
            for rel in p["pkgbuild"]:
                f = repo_dir(pathlib.Path(argv[2]), p) / rel
                if f.is_file():
                    f.write_text(sync_text(f.read_text(), line(m, name)))
                    print(f"compat: {f}")
    elif cmd == "releases":
        root = pathlib.Path(argv[2])
        rel = {n: release_of(repo_dir(root, p)) for n, p in m["package"].items()}
        errs = unmet(m, {n: v for n, v in rel.items() if v})
        print("compat: latest releases " + ", ".join(f"{n} {v}" for n, v in sorted(rel.items())))
    elif cmd == "aur":
        names = sorted({a for p in m["package"].values() for a in p["aur"]})
        errs, have = aur_problems(m, aur_info(names))
        on_aur = {n: have[p["aur"][0]].split("-")[0] for n, p in m["package"].items() if p["aur"][0] in have}
        print("compat: on the AUR " + ", ".join(f"{n} {v}" for n, v in sorted(have.items())))
        if len(argv) > 2:
            # Behind is normal for a while after a release (the AUR is pushed by hand): said, not failed.
            root = pathlib.Path(argv[2])
            for n, p in sorted(m["package"].items()):
                rel = release_of(repo_dir(root, p))
                if rel and n in on_aur and vkey(on_aur[n]) < vkey(rel):
                    print(f"compat: behind: {n} {rel} is released, the AUR has {on_aur[n]}")
    else:
        print(__doc__, file=sys.stderr)
        return 2
    for e in errs:
        print(f"compat: {e}", file=sys.stderr)
    return 1 if errs else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
