// Tests for the generated QML theme base: fallbacks off uQ, tokens on uQ, Reduce motion.
import QtQuick
import QtTest

TestCase {
    name: "UqThemeBase"

    UqThemeBase { id: off }
    UqThemeBase {
        id: on
        tokens: ({ present: true, variant: "light", radius: 3, fontSize: 14, fontUi: "Inter",
                   colors: { bg: "#ffffff", accent: "#123456" },
                   motion: { fast: 90, base: 130, slow: 200, reduce: false } })
    }
    UqThemeBase {
        id: reduced
        tokens: ({ present: true, colors: {}, motion: { fast: 0, base: 0, slow: 0, reduce: true } })
    }
    UqThemeBase { id: desk; fallbackColors: ({ bg: "#eff0f1" }); fallbackRadius: 6 }
    UqThemeBase {
        id: deskOnUq
        fallbackColors: ({ bg: "#eff0f1" })
        tokens: ({ present: true, colors: { bg: "#000001" } })
    }
    UqThemeBase { id: oldUq; tokens: ({ present: true, colors: {}, radius: 1 }) }

    function test_fallbacks() {
        compare(off.uq, false)
        compare(off.dark, true)
        compare(off.bg, "#1f1f22")
        compare(off.radius, 10)
        compare(off.radiusControl, 8)
        compare(off.motionBase, 140)
        compare(off.easing, Easing.OutCubic)
        verify(off.bodySize > 0)
        compare(off.headingSize, Math.round(off.bodySize * 1.5))
    }

    function test_tokens() {
        compare(on.dark, false)
        compare(on.bg, "#ffffff")
        compare(on.accent, "#123456")
        compare(on.surface, "#2b2b2e")
        compare(on.radius, 3)
        compare(on.radiusControl, 1)
        compare(on.radiusInner, 0)
        compare(on.bodySize, 14)
        compare(on.smallSize, 13)
        compare(on.headingSize, 21)
        compare(on.motionFast, 90)
        compare(on.popScale, 0.96)
    }

    function test_an_app_s_own_fallbacks_apply_only_off_uq() {
        compare(desk.bg, "#eff0f1")
        compare(desk.surface, "#2b2b2e")
        compare(desk.radius, 6)
        compare(desk.radiusControl, 4)
        compare(desk.dark, false)
        compare(deskOnUq.bg, "#000001")
    }

    function test_reduce_motion() {
        compare(reduced.reduceMotion, true)
        compare(reduced.motionFast, 0)
        compare(reduced.motionSlow, 0)
        compare(reduced.popScale, 1)
    }

    function test_an_older_uq_without_motion_keeps_the_defaults() {
        compare(oldUq.motionFast, 100)
        compare(oldUq.radiusControl, 0)
    }
}
