// uqinput: a virtual keyboard and pointer for the live checks (tests/live, scripts/drag-live.sh).
// Hyprland has no input-injection command, and ydotool needs /dev/uinput, which a CI container
// doesn't have; the wlr virtual-pointer and virtual-keyboard protocols need nothing.
//
//   uqinput WIDTH HEIGHT < commands
//
// WIDTH x HEIGHT is the layout size of the output the pointer moves over. One command a line,
// each done and flushed (a round trip to the compositor) before the next is read:
//   move X Y          the pointer to X,Y (layout pixels)
//   button N down|up  a pointer button (272 left, 273 right)
//   mod super down|up the Super key, with the modifier state (shift, ctrl and alt likewise)
//   key NAME          press and release a key by its xkb keysym name (Return, Escape, Down, 1, …)
//   type TEXT         type the rest of the line (US layout, shift where a character needs it)
//   sleep MS
//   # text            ignored
// Prints "ready" once both devices exist, and "done" at end of input. Exit 1 on a protocol error.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>
#include <wayland-client.h>
#include <xkbcommon/xkbcommon.h>
#include "virtual-keyboard-unstable-v1-client-protocol.h"
#include "wlr-virtual-pointer-unstable-v1-client-protocol.h"

static struct wl_seat *seat;
static struct zwp_virtual_keyboard_manager_v1 *kbd_mgr;
static struct zwlr_virtual_pointer_manager_v1 *ptr_mgr;

static void global(void *d, struct wl_registry *r, uint32_t name, const char *iface, uint32_t ver) {
    (void)d;
    if (!strcmp(iface, wl_seat_interface.name))
        seat = wl_registry_bind(r, name, &wl_seat_interface, 1);
    else if (!strcmp(iface, zwp_virtual_keyboard_manager_v1_interface.name))
        kbd_mgr = wl_registry_bind(r, name, &zwp_virtual_keyboard_manager_v1_interface, 1);
    else if (!strcmp(iface, zwlr_virtual_pointer_manager_v1_interface.name))
        ptr_mgr = wl_registry_bind(r, name, &zwlr_virtual_pointer_manager_v1_interface, ver < 2 ? ver : 2);
}
static void global_remove(void *d, struct wl_registry *r, uint32_t n) { (void)d; (void)r; (void)n; }
static const struct wl_registry_listener reg = { global, global_remove };

static uint32_t now_ms(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (uint32_t)(t.tv_sec * 1000 + t.tv_nsec / 1000000);
}

// Press and release the key that makes `sym` on the default layout, with shift when it takes level 2.
static int press_sym(struct wl_display *dpy, struct zwp_virtual_keyboard_v1 *kbd, struct xkb_keymap *km,
                     xkb_keysym_t sym, uint32_t depressed, uint32_t shift_bit) {
    xkb_keycode_t lo = xkb_keymap_min_keycode(km), hi = xkb_keymap_max_keycode(km);
    for (xkb_keycode_t kc = lo; kc <= hi; kc++) {
        for (xkb_level_index_t lv = 0; lv < 2; lv++) {
            const xkb_keysym_t *syms;
            if (xkb_keymap_key_get_syms_by_level(km, kc, 0, lv, &syms) < 1 || syms[0] != sym) continue;
            uint32_t mods = depressed | (lv == 1 ? shift_bit : 0);
            zwp_virtual_keyboard_v1_modifiers(kbd, mods, 0, 0, 0);
            zwp_virtual_keyboard_v1_key(kbd, now_ms(), kc - 8, 1);
            wl_display_roundtrip(dpy);
            usleep(12000);
            zwp_virtual_keyboard_v1_key(kbd, now_ms(), kc - 8, 0);
            zwp_virtual_keyboard_v1_modifiers(kbd, depressed, 0, 0, 0);
            return 1;
        }
    }
    return 0;
}

int main(int argc, char **argv) {
    if (argc != 3) { fprintf(stderr, "usage: uqinput WIDTH HEIGHT < commands\n"); return 2; }
    uint32_t W = (uint32_t)atoi(argv[1]), H = (uint32_t)atoi(argv[2]);
    struct wl_display *dpy = wl_display_connect(NULL);
    if (!dpy) { fprintf(stderr, "uqinput: no Wayland display\n"); return 1; }
    struct wl_registry *r = wl_display_get_registry(dpy);
    wl_registry_add_listener(r, &reg, NULL);
    wl_display_roundtrip(dpy);
    if (!seat || !kbd_mgr || !ptr_mgr) {
        fprintf(stderr, "uqinput: the compositor lacks a seat, virtual keyboard or virtual pointer\n");
        return 1;
    }
    struct zwp_virtual_keyboard_v1 *kbd = zwp_virtual_keyboard_manager_v1_create_virtual_keyboard(kbd_mgr, seat);
    struct zwlr_virtual_pointer_v1 *ptr = zwlr_virtual_pointer_manager_v1_create_virtual_pointer(ptr_mgr, seat);

    // The keymap: whatever xkbcommon's defaults give (US), written to a memfd.
    struct xkb_context *ctx = xkb_context_new(XKB_CONTEXT_NO_FLAGS);
    struct xkb_keymap *km = xkb_keymap_new_from_names(ctx, NULL, XKB_KEYMAP_COMPILE_NO_FLAGS);
    if (!km) { fprintf(stderr, "uqinput: no default xkb keymap\n"); return 1; }
    char *text = xkb_keymap_get_as_string(km, XKB_KEYMAP_FORMAT_TEXT_V1);
    size_t size = strlen(text) + 1;
    int fd = memfd_create("keymap", 0);
    if (fd < 0 || ftruncate(fd, (off_t)size) < 0) { perror("uqinput: keymap"); return 1; }
    void *map = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    memcpy(map, text, size);
    munmap(map, size);
    zwp_virtual_keyboard_v1_keymap(kbd, WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1, fd, (uint32_t)size);

    struct { const char *name, *xkb; uint32_t key; } mods[] = {
        { "super", "Mod4", 125 }, { "shift", "Shift", 42 }, { "ctrl", "Control", 29 }, { "alt", "Mod1", 56 },
    };
    uint32_t depressed = 0;
    wl_display_roundtrip(dpy);
    puts("ready");
    fflush(stdout);

    char line[256];
    while (fgets(line, sizeof line, stdin)) {
        char a[32] = "", b[32] = "";
        int x = 0, y = 0;
        if (line[0] == '#' || line[0] == '\n') continue;
        if (sscanf(line, "move %d %d", &x, &y) == 2) {
            zwlr_virtual_pointer_v1_motion_absolute(ptr, now_ms(), (uint32_t)x, (uint32_t)y, W, H);
            zwlr_virtual_pointer_v1_frame(ptr);
        } else if (sscanf(line, "button %d %31s", &x, b) == 2) {
            zwlr_virtual_pointer_v1_button(ptr, now_ms(), (uint32_t)x,
                !strcmp(b, "down") ? WL_POINTER_BUTTON_STATE_PRESSED : WL_POINTER_BUTTON_STATE_RELEASED);
            zwlr_virtual_pointer_v1_frame(ptr);
        } else if (sscanf(line, "mod %31s %31s", a, b) == 2) {
            int found = 0, down = !strcmp(b, "down");
            for (size_t i = 0; i < sizeof mods / sizeof *mods; i++) {
                if (strcmp(mods[i].name, a)) continue;
                xkb_mod_index_t idx = xkb_keymap_mod_get_index(km, mods[i].xkb);
                uint32_t bit = idx == XKB_MOD_INVALID ? 0 : 1u << idx;
                if (down) depressed |= bit; else depressed &= ~bit;
                zwp_virtual_keyboard_v1_key(kbd, now_ms(), mods[i].key, down ? 1 : 0);
                zwp_virtual_keyboard_v1_modifiers(kbd, depressed, 0, 0, 0);
                found = 1;
            }
            if (!found) { fprintf(stderr, "uqinput: no modifier %s\n", a); return 1; }
        } else if (!strncmp(line, "key ", 4)) {
            char name[64] = "";
            sscanf(line + 4, "%63s", name);
            xkb_keysym_t sym = xkb_keysym_from_name(name, XKB_KEYSYM_NO_FLAGS);
            xkb_mod_index_t si = xkb_keymap_mod_get_index(km, "Shift");
            if (sym == XKB_KEY_NoSymbol || !press_sym(dpy, kbd, km, sym, depressed, si == XKB_MOD_INVALID ? 0 : 1u << si)) {
                fprintf(stderr, "uqinput: no key %s\n", name);
                return 1;
            }
        } else if (!strncmp(line, "type ", 5)) {
            xkb_mod_index_t si = xkb_keymap_mod_get_index(km, "Shift");
            for (const char *c = line + 5; *c && *c != '\n'; c++) {
                xkb_keysym_t sym = xkb_utf32_to_keysym((uint32_t)(unsigned char)*c);
                if (!press_sym(dpy, kbd, km, sym, depressed, si == XKB_MOD_INVALID ? 0 : 1u << si)) {
                    fprintf(stderr, "uqinput: can't type %c\n", *c);
                    return 1;
                }
            }
        } else if (sscanf(line, "sleep %d", &x) == 1) {
            usleep((useconds_t)x * 1000);
        } else {
            fprintf(stderr, "uqinput: can't read: %s", line);
            return 1;
        }
        if (wl_display_roundtrip(dpy) < 0) { fprintf(stderr, "uqinput: the compositor hung up\n"); return 1; }
        puts("ok");
        fflush(stdout);
    }
    puts("done");
    return 0;
}
