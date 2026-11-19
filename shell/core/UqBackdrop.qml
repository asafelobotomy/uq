import QtQuick

// uQ's own backdrop: soft facets in the theme's colours with the uQ mark (a Q whose bowl holds a
// u) at the centre. Used where no wallpaper is set (the desktop and the login screen), so it
// follows the theme, dark or light, with no picture to ship. Drawn once per size or theme change.
Item {
    id: root

    property color bg: Theme.bg
    property color surface: Theme.surface
    property color surfaceAlt: Theme.surfaceAlt
    property color accent: Theme.accent
    property color text: Theme.text
    property bool showMark: true

    // Triangles as [x1, y1, x2, y2, x3, y3, colour, alpha], in fractions of the width and height
    // (the x extent is squeezed by the width, so the shapes keep their look on any screen). Colour
    // 0 is surface, 1 surface-alt, 2 accent.
    readonly property var facets: [
        [0.389, 0.097, 0.134, 0.128, 0.246, -0.282, 2, 0.09],
        [0.621, 0.978, 0.459, 0.973, 0.543, 0.724, 1, 0.08],
        [0.12, 0.521, -0.163, 0.39, 0.042, 0.017, 0, 0.18],
        [0.674, -0.05, 0.438, 0.041, 0.512, -0.369, 0, 0.08],
        [0.907, 0.25, 0.763, 0.194, 0.862, -0.001, 1, 0.17],
        [0.703, 0.151, 0.514, -0.109, 0.734, -0.271, 0, 0.16],
        [-0.07, -0.278, 0.061, -0.115, -0.084, 0.007, 1, 0.2],
        [0.413, 1.108, 0.315, 0.81, 0.508, 0.806, 0, 0.19],
        [0.262, 0.322, 0.203, 0.717, 0.041, 0.429, 2, 0.15],
        [0.636, 0.088, 0.44, -0.109, 0.634, -0.315, 1, 0.1],
        [0.451, -0.419, 0.56, 0.001, 0.302, -0.04, 2, 0.2],
        [0.69, 0.213, 0.853, 0.035, 0.858, 0.376, 1, 0.08],
        [0.143, 0.232, -0.12, 0.287, -0.015, -0.148, 2, 0.19],
        [0.576, 1.286, 0.487, 0.789, 0.772, 0.901, 1, 0.22],
        [0.196, 0.802, 0.386, 0.86, 0.263, 1.124, 1, 0.08],
        [0.669, 0.066, 0.729, -0.217, 0.836, 0.017, 1, 0.08],
        [0.468, 0.174, 0.496, 0.716, 0.219, 0.488, 2, 0.12],
        [0.527, 0.147, 0.312, 0.535, 0.231, 0.01, 0, 0.08],
        [0.1, 0.483, 0.102, 0.695, -0.002, 0.592, 0, 0.11],
        [-0.199, 0.222, -0.005, 0.184, -0.083, 0.503, 0, 0.19],
        [0.609, 0.631, 0.353, 0.716, 0.439, 0.277, 2, 0.21],
        [0.278, 0.172, 0.41, 0.25, 0.307, 0.414, 0, 0.1],
        [0.918, 0.244, 1.057, 0.27, 0.975, 0.472, 0, 0.07],
        [0.177, -0.046, -0.004, 0.077, 0.026, -0.265, 0, 0.17],
        [-0.009, 0.251, 0.028, -0.083, 0.171, 0.14, 0, 0.09],
        [0.389, 1.182, 0.359, 0.795, 0.562, 0.943, 0, 0.09]
    ]

    function paint(ctx, w, h) {
        ctx.reset();
        ctx.fillStyle = root.bg;
        ctx.fillRect(0, 0, w, h);
        var cols = [root.surface, root.surfaceAlt, root.accent];
        for (var i = 0; i < facets.length; i++) {
            var f = facets[i];
            ctx.globalAlpha = f[7] * (f[6] === 2 ? 0.55 : 1.4);
            ctx.fillStyle = cols[f[6]];
            ctx.beginPath();
            ctx.moveTo(f[0] * w, f[1] * h);
            ctx.lineTo(f[2] * w, f[3] * h);
            ctx.lineTo(f[4] * w, f[5] * h);
            ctx.closePath();
            ctx.fill();
        }
        ctx.globalAlpha = 1;
        if (!root.showMark)
            return;
        var r = Math.min(w, h) * 0.12, cx = w / 2, cy = h * 0.46, lw = Math.max(2, r * 0.1);
        ctx.strokeStyle = root.accent;
        ctx.lineWidth = lw;
        ctx.lineCap = "butt";
        ctx.beginPath();
        ctx.arc(cx, cy, r, 0, 2 * Math.PI);
        ctx.stroke();
        var a = Math.PI / 4;
        ctx.beginPath();
        ctx.moveTo(cx + Math.cos(a) * r * 0.55, cy + Math.sin(a) * r * 0.55);
        ctx.lineTo(cx + Math.cos(a) * r * 1.3, cy + Math.sin(a) * r * 1.3);
        ctx.stroke();
        // The u: two stems and a half circle.
        var uw = r * 0.62, top = cy - r * 0.38, bot = cy + r * 0.3;
        ctx.lineCap = "square";
        ctx.beginPath();
        ctx.moveTo(cx - uw / 2, top);
        ctx.lineTo(cx - uw / 2, bot - uw / 2);
        ctx.arc(cx, bot - uw / 2, uw / 2, Math.PI, 0, true);
        ctx.moveTo(cx + uw / 2, bot - uw / 2);
        ctx.lineTo(cx + uw / 2, top);
        ctx.stroke();
    }

    Canvas {
        id: canvas
        anchors.fill: parent
        renderTarget: Canvas.Image
        onPaint: root.paint(getContext("2d"), width, height)
    }
    onBgChanged: canvas.requestPaint()
    onSurfaceChanged: canvas.requestPaint()
    onSurfaceAltChanged: canvas.requestPaint()
    onAccentChanged: canvas.requestPaint()
    onShowMarkChanged: canvas.requestPaint()
}
