import QtQuick
import QtTest
import "../../shell/logic/fit.js" as Fit
import "../../shell/logic/shellprefs.js" as ShellPrefs

TestCase {
    name: "fit"

    // eDP-1 as a real session reports it: 1920x1080, the bar's 60 px reserved at the top.
    readonly property var laptop: Fit.monitorFromIpc({ id: 0, name: "eDP-1", x: 0, y: 0,
        width: 1920, height: 1080, scale: 1, transform: 0, reserved: [0, 60, 0, 0] })

    function win(x, y, w, h, extra) {
        var o = { address: "0x55632dacbbf0", at: [x, y], size: [w, h], floating: true,
                  hidden: false, fullscreen: 0, monitor: 0 };
        for (var k in (extra || {}))
            o[k] = extra[k];
        return Fit.windowFromIpc(o);
    }

    function test_from_ipc() {
        compare(Fit.windowFromIpc(null), null);
        compare(Fit.windowFromIpc({ address: "0x1", at: [0, 0] }), null);
        compare(Fit.windowFromIpc({ address: "nope", at: [0, 0], size: [1, 1] }), null);
        var w = win(-800, -350, 2500, 1500);
        compare([w.x, w.y, w.w, w.h, w.floating], [-800, -350, 2500, 1500, true]);
        compare([laptop.x, laptop.w, laptop.h], [0, 1920, 1080]);
        compare(laptop.reserved, [0, 60, 0, 0]);
        // Scaled and turned screens give their logical size.
        var hi = Fit.monitorFromIpc({ id: 1, x: 1920, y: 0, width: 3840, height: 2160, scale: 2, transform: 1 });
        compare([hi.x, hi.w, hi.h], [1920, 1080, 1920]);
        compare(hi.reserved, [0, 0, 0, 0]);
        compare(Fit.monitorFromIpc({ id: 2, width: 0, height: 0 }), null);
        compare(Fit.monitorFromIpc({ id: 2, width: 10, height: 10, disabled: true }), null);
    }

    function test_too_big_is_shrunk_inside() {
        // What Hyprland did with a 2500x1500 floating window: centred, hanging off every edge.
        compare(Fit.placement(win(-800, -350, 2500, 1500), laptop, 12),
                { x: 12, y: 72, w: 1896, h: 996 });
        // Too tall only: keeps its width and its x.
        compare(Fit.placement(win(300, 100, 800, 1400), laptop, 12),
                { x: 300, y: 72, w: 800, h: 996 });
    }

    function test_hanging_over_an_edge_moves_back() {
        // Under the bar.
        compare(Fit.placement(win(300, 20, 800, 600), laptop, 12), { x: 300, y: 72, w: 800, h: 600 });
        // Off the right and bottom.
        compare(Fit.placement(win(1500, 900, 800, 600), laptop, 12), { x: 1108, y: 468, w: 800, h: 600 });
    }

    function test_left_alone() {
        // Wholly inside, even flush with the bar or the screen's edge.
        compare(Fit.placement(win(0, 60, 800, 600), laptop, 12), null);
        compare(Fit.placement(win(1120, 480, 800, 600), laptop, 12), null);
        compare(Fit.placement(win(300, 59, 800, 600), laptop, 12), null, "a pixel over is fine");
        // Tiled, fullscreen, hidden: not these rules' business.
        compare(Fit.placement(win(-800, -350, 2500, 1500, { floating: false }), laptop, 12), null);
        compare(Fit.placement(win(-800, -350, 2500, 1500, { fullscreen: 2 }), laptop, 12), null);
        compare(Fit.placement(win(-800, -350, 2500, 1500, { hidden: true }), laptop, 12), null);
        compare(Fit.placement(win(-800, -350, 2500, 1500), null, 12), null);
    }

    function test_monitor_for() {
        var right = Fit.monitorFromIpc({ id: 1, x: 1920, y: 0, width: 2560, height: 1440, scale: 1, reserved: [0, 0, 0, 0] });
        var mons = [laptop, right];
        compare(Fit.monitorFor(win(2000, 100, 800, 600, { monitor: 0 }), mons).id, 1, "by its centre");
        compare(Fit.monitorFor(win(-5000, 100, 800, 600, { monitor: 1 }), mons).id, 1, "else Hyprland's");
        compare(Fit.monitorFor(win(-5000, 100, 800, 600, { monitor: 7 }), mons), null);
        // On the second screen it fits that screen.
        compare(Fit.placement(win(4000, 100, 800, 600), right, 12), { x: 3668, y: 100, w: 800, h: 600 });
    }

    function test_events() {
        compare(Fit.eventAddress("openwindow", "55632dacbbf0,1,kitty,Kitty"), "0x55632dacbbf0");
        compare(Fit.eventAddress("changefloatingmode", "55632dacbbf0,1"), "0x55632dacbbf0");
        compare(Fit.eventAddress("movewindowv2", "55632dacbbf0,2,2"), "0x55632dacbbf0");
        compare(Fit.eventAddress("closewindow", "55632dacbbf0"), null);
        compare(Fit.eventAddress("openwindow", "zz,1"), null);
        verify(Fit.screensChanged("monitoraddedv2"));
        verify(!Fit.screensChanged("openwindow"));
    }

    function test_setting() {
        verify(ShellPrefs.fitWindows(null));
        verify(ShellPrefs.fitWindows({}));
        verify(ShellPrefs.fitWindows({ fit_windows: "no" }));
        verify(!ShellPrefs.fitWindows({ fit_windows: false }));
    }
}
