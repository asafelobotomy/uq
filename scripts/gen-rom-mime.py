#!/usr/bin/env python3
"""Generate uQ's ROM and retro-platform file types and their icons.

Writes packaging/mime/uq-roms.xml (a shared-mime-info package file) and one scalable icon per
platform in packaging/mime/icons/. Both are committed; rerun this after editing PLATFORMS:

    python3 scripts/gen-rom-mime.py

Types that shared-mime-info already defines (application/x-nes-rom and friends) are reused, never
renamed: this file only adds extensions, a plainer name and an icon to them. Extensions that mean
something else too (.bin, .iso, .rom, .md, .st, .img, .o, .p) are never claimed. Plain .iso discs
(PlayStation, PSP, Xbox 360...) can't be told apart by name, and Qt and KIO only read the first
16 KiB, where a disc's platform marker (offset 32776) isn't, so they keep their generic disc type.
"""
import os
import sys
from xml.sax.saxutils import escape

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "packaging", "mime")

# (slug, mime type, name, label on the icon, shape, accent colour, extensions, magic, reuse)
# shape: cart (cartridge), disc (optical disc), card (card, tape or digital package)
# magic: (offset, bytes, priority) or None. reuse: the type already exists upstream.
PLATFORMS = [
    ("nes", "application/x-nes-rom", "Nintendo Entertainment System ROM", "NES", "cart", "#d0021b", ["nes", "unf", "unif", "nez"], None, True),
    ("fds", "application/x-fds-disk", "Famicom Disk System disk", "FDS", "card", "#d0021b", ["fds", "qd"], None, True),
    ("snes", "application/vnd.nintendo.snes.rom", "Super Nintendo ROM", "SNES", "cart", "#7b5ea7", ["sfc", "smc", "swc", "fig"], None, True),
    ("n64", "application/x-n64-rom", "Nintendo 64 ROM", "N64", "cart", "#2e9e4f", ["n64", "z64", "v64"], None, True),
    ("gb", "application/x-gameboy-rom", "Game Boy ROM", "GB", "cart", "#7a8f3a", ["gb", "sgb"], None, True),
    ("gbc", "application/x-gameboy-color-rom", "Game Boy Color ROM", "GBC", "cart", "#8e44ad", ["gbc", "cgb"], None, True),
    ("gba", "application/x-gba-rom", "Game Boy Advance ROM", "GBA", "cart", "#3b5bdb", ["gba", "agb"], None, True),
    ("vb", "application/x-virtual-boy-rom", "Virtual Boy ROM", "VB", "cart", "#c0182b", ["vb", "vboy"], None, True),
    ("pokemini", "application/x-pokemon-mini-rom", "Pokémon Mini ROM", "MINI", "cart", "#e0a800", ["min"], None, False),
    ("nds", "application/x-nintendo-ds-rom", "Nintendo DS ROM", "NDS", "card", "#6c757d", ["nds", "dsi"], None, True),
    ("3ds", "application/x-nintendo-3ds-rom", "Nintendo 3DS ROM", "3DS", "card", "#d0021b", ["3ds", "cci", "cia", "cxi", "zcci", "zcia", "zcxi"], None, True),
    ("gc", "application/x-gamecube-rom", "GameCube disc image", "GC", "disc", "#5f3dc4", ["gcm", "gcz", "rvz", "ciso"], None, True),
    ("wii", "application/x-wii-rom", "Wii disc image", "Wii", "disc", "#1c8fc7", ["wbfs", "wia"], None, True),
    ("wiiu", "application/x-wiiu-rom", "Wii U disc image", "WiiU", "disc", "#1098ad", ["wud", "wux", "wuhb"], None, False),
    ("switch", "application/x-nintendo-switch-rom", "Nintendo Switch game", "NSW", "card", "#e60012", ["nsp", "xci"], None, False),
    ("sg1000", "application/x-sg1000-rom", "SG-1000 ROM", "SG", "cart", "#1864ab", ["sg"], None, True),
    ("sms", "application/x-sms-rom", "Master System ROM", "SMS", "cart", "#1864ab", ["sms"], None, True),
    ("genesis", "application/x-genesis-rom", "Genesis ROM", "MD", "cart", "#0b5cad", ["gen", "smd", "sgd"], None, True),
    ("32x", "application/x-genesis-32x-rom", "Sega 32X ROM", "32X", "cart", "#0b5cad", ["32x", "mdx"], None, True),
    ("gg", "application/x-gamegear-rom", "Game Gear ROM", "GG", "cart", "#1864ab", ["gg"], None, True),
    ("pico", "application/x-sega-pico-rom", "Sega Pico ROM", "PICO", "cart", "#1864ab", [], None, True),
    ("segacd", "application/x-sega-cd-rom", "Sega CD disc image", "SCD", "disc", "#0b5cad", [], None, True),
    ("saturn", "application/x-saturn-rom", "Sega Saturn disc image", "SAT", "disc", "#495057", [], None, True),
    ("dreamcast", "application/x-dreamcast-rom", "Dreamcast disc image", "DC", "disc", "#e8590c", ["gdi"], None, True),
    ("pbp", "application/x-playstation-pbp", "PlayStation game package", "PBP", "card", "#2b6fd6", ["pbp"], None, False),
    ("vita", "application/x-psvita-vpk", "PlayStation Vita game", "VITA", "card", "#2b6fd6", ["vpk"], None, False),
    ("xbox", "application/x-xbox-iso", "Xbox disc image", "XBOX", "disc", "#2f9e44", ["xiso"], None, False),
    ("atari2600", "application/x-atari-2600-rom", "Atari 2600 ROM", "2600", "cart", "#d9480f", ["a26"], None, True),
    ("atari5200", "application/x-atari-5200-rom", "Atari 5200 ROM", "5200", "cart", "#d9480f", ["a52"], None, False),
    ("atari7800", "application/x-atari-7800-rom", "Atari 7800 ROM", "7800", "cart", "#d9480f", ["a78"], None, True),
    ("lynx", "application/x-atari-lynx-rom", "Atari Lynx ROM", "LYNX", "cart", "#d9480f", ["lnx", "lyx"], None, True),
    ("jaguar", "application/x-atari-jaguar-rom", "Atari Jaguar ROM", "JAG", "cart", "#d9480f", ["j64", "jag"], None, False),
    ("pce", "application/x-pc-engine-rom", "PC Engine ROM", "PCE", "card", "#e67700", ["pce", "sgx"], None, True),
    ("ngp", "application/x-neo-geo-pocket-rom", "Neo Geo Pocket ROM", "NGP", "cart", "#343a40", ["ngp"], None, True),
    ("ngpc", "application/x-neo-geo-pocket-color-rom", "Neo Geo Pocket Color ROM", "NGPC", "cart", "#343a40", ["ngc"], None, True),
    ("ws", "application/x-wonderswan-rom", "WonderSwan ROM", "WS", "cart", "#0c8599", ["ws"], None, True),
    ("wsc", "application/x-wonderswan-color-rom", "WonderSwan Color ROM", "WSC", "cart", "#0c8599", ["wsc"], None, True),
    ("msx", "application/x-msx-rom", "MSX ROM", "MSX", "cart", "#364fc7", ["msx", "mx1", "mx2"], None, True),
    ("coleco", "application/x-colecovision-rom", "ColecoVision ROM", "COL", "cart", "#c2255c", ["col"], None, False),
    ("intv", "application/x-intellivision-rom", "Intellivision ROM", "INTV", "cart", "#a61e4d", ["int"], None, False),
    ("vectrex", "application/x-vectrex-rom", "Vectrex ROM", "VEC", "cart", "#212529", ["vec", "gam"], None, False),
    ("channelf", "application/x-channelf-rom", "Fairchild Channel F ROM", "CHF", "cart", "#5c940d", ["chf"], None, False),
    ("sv", "application/x-supervision-rom", "Watara Supervision ROM", "SV", "cart", "#087f5b", ["sv"], None, False),
    ("pc2", "application/x-pocket-challenge-rom", "Pocket Challenge V2 ROM", "PC2", "cart", "#0c8599", ["pc2"], None, False),
    ("c64", "application/x-commodore-64-disk", "Commodore 64 disk or tape", "C64", "card", "#7048e8", ["d64", "t64", "g64"], None, False),
    ("amiga", "application/x-amiga-disk-format", "Amiga disk image", "AMI", "card", "#e8590c", ["adf", "ipf", "hdf"], None, True),
    ("zxs", "application/x-zx-spectrum-tape", "ZX Spectrum tape or snapshot", "ZXS", "card", "#c92a2a", ["tzx", "z80", "sna"], None, False),
    ("atari8", "application/x-atari-8bit-disk", "Atari 8-bit disk or cartridge", "A8", "card", "#d9480f", ["atr", "xex", "xfd"], None, False),
    ("pc98", "application/x-pc98-disk", "NEC PC-98 disk image", "PC98", "card", "#e67700", ["hdi", "fdi", "d88", "nhd"], None, False),
    ("cue", "application/x-cue", None, "CUE", "disc", "#5c6370", [], None, True),
    ("chd", "application/x-mame-chd", "Compressed disc image (CHD)", "CHD", "disc", "#5c6370", [], None, True),
    ("neogeo", "application/x-neo-geo-rom", "Neo Geo ROM", "NEO", "cart", "#c92a2a", ["neo"], None, False),
]

SHAPES = {
    # Cartridge: shell with a label sticker above the contacts.
    "cart": """<path d="M12 8a4 4 0 0 1 4-4h32a4 4 0 0 1 4 4v44a4 4 0 0 1-4 4H16a4 4 0 0 1-4-4z" fill="#3d4350"/>
  <path d="M12 8a4 4 0 0 1 4-4h32a4 4 0 0 1 4 4v6H12z" fill="#555c6b"/>
  <rect x="17" y="17" width="30" height="24" rx="3" fill="{accent}"/>
  <rect x="18" y="46" width="28" height="9" rx="1.5" fill="#1f232b"/>
  <path d="M21 48v5M26 48v5M31 48v5M36 48v5M41 48v5" stroke="#c9a227" stroke-width="2"/>""",
    # Card: rounded slab with a notched corner, for handheld carts and digital packages.
    "card": """<path d="M10 8a4 4 0 0 1 4-4h30l10 10v38a4 4 0 0 1-4 4H14a4 4 0 0 1-4-4z" fill="#3d4350"/>
  <path d="M44 4l10 10H48a4 4 0 0 1-4-4z" fill="#555c6b"/>
  <rect x="15" y="19" width="34" height="24" rx="3" fill="{accent}"/>
  <rect x="17" y="48" width="30" height="6" rx="1.5" fill="#1f232b"/>
  <path d="M21 50v2M25 50v2M29 50v2M33 50v2M37 50v2M41 50v2" stroke="#c9a227" stroke-width="1.6"/>""",
    # Disc: platter with the label band across the centre.
    "disc": """<circle cx="32" cy="32" r="28" fill="#3d4350"/>
  <circle cx="32" cy="32" r="26" fill="none" stroke="#5a6171" stroke-width="1.5"/>
  <circle cx="32" cy="32" r="9" fill="#1f232b"/>
  <circle cx="32" cy="32" r="3.5" fill="#d7dae0"/>
  <path d="M10 17a28 28 0 0 1 18-12" fill="none" stroke="#ffffff" stroke-opacity=".28" stroke-width="3" stroke-linecap="round"/>
  <rect x="6" y="24" width="52" height="16" rx="3" fill="{accent}"/>""",
}
# Where the platform label sits on each shape (centre x, baseline y, available width).
LABELS = {"cart": (32, 33.5, 28), "card": (32, 35.5, 32), "disc": (32, 36.4, 50)}


def luminance(hexcolour):
    r, g, b = (int(hexcolour[i:i + 2], 16) / 255 for i in (1, 3, 5))
    lin = [c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4 for c in (r, g, b)]
    return 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2]


def icon_svg(label, shape, accent):
    cx, base, width = LABELS[shape]
    size = min(13, width / (0.66 * len(label)))
    ink = "#14171c" if luminance(accent) > 0.35 else "#ffffff"
    body = SHAPES[shape].format(accent=accent)
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64">\n'
        f"  {body}\n"
        f'  <text x="{cx}" y="{base}" text-anchor="middle" font-family="Inter, Noto Sans, DejaVu Sans, sans-serif" '
        f'font-weight="700" font-size="{size:.1f}" fill="{ink}">{escape(label)}</text>\n'
        "</svg>\n"
    )


def mime_xml():
    out = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        "<!-- Generated by scripts/gen-rom-mime.py; edit that, not this. -->",
        '<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">',
    ]
    for slug, mime, name, label, shape, accent, exts, magic, reuse in PLATFORMS:
        out.append(f'  <mime-type type="{mime}">')
        if name:
            out.append(f"    <comment>{escape(name)}</comment>")
        out.append(f'    <icon name="uq-rom-{slug}"/>')
        out.append('    <generic-icon name="applications-games"/>')
        if magic:
            off, text, prio = magic
            out.append(f'    <magic priority="{prio}"><match type="string" offset="{off}" value="{escape(text)}"/></magic>')
        for e in exts:
            out.append(f'    <glob pattern="*.{e}" weight="60"/>')
        out.append("  </mime-type>")
    out.append("</mime-info>")
    return "\n".join(out) + "\n"


def main():
    icons = os.path.join(OUT, "icons")
    os.makedirs(icons, exist_ok=True)
    wanted = set()
    for slug, _, _, label, shape, accent, *_ in PLATFORMS:
        wanted.add(f"uq-rom-{slug}.svg")
        with open(os.path.join(icons, f"uq-rom-{slug}.svg"), "w") as f:
            f.write(icon_svg(label, shape, accent))
    for stale in set(os.listdir(icons)) - wanted:
        os.remove(os.path.join(icons, stale))
    with open(os.path.join(OUT, "uq-roms.xml"), "w") as f:
        f.write(mime_xml())
    print(f"{len(PLATFORMS)} platform types")


if __name__ == "__main__":
    sys.exit(main())
