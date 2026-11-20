.pragma library

// The screenshot preview's pure rules (ShotPreview.qml owns the window, the timer and the
// processes). `uq-screenshot` hands the shell the saved file's path over IPC; nothing from the
// path is ever put in a shell command line (the processes get it as a separate argument).

var SHOW_MS = 12000;

// An absolute path to a PNG with no ".." part and no control characters.
function validPath(p) {
    if (typeof p !== "string" || p.length < 2 || p.length > 4096)
        return false;
    if (p.charAt(0) !== "/" || !/\.png$/i.test(p))
        return false;
    if (/[\u0000-\u001f\u007f]/.test(p))
        return false;
    return p.split("/").indexOf("..") < 0;
}

// The file as an Image `source`.
function fileUrl(p) {
    return "file://" + encodeURI(p).replace(/#/g, "%23").replace(/\?/g, "%3F");
}

function baseName(p) {
    var i = String(p).lastIndexOf("/");
    return i < 0 ? String(p) : String(p).slice(i + 1);
}

// "/home/me/Pictures/Screenshots/x.png" -> "~/Pictures/Screenshots" (the folder, for the caption).
function folderLabel(p, home) {
    var i = String(p).lastIndexOf("/");
    var dir = i <= 0 ? "/" : String(p).slice(0, i);
    if (home && (dir === home || dir.indexOf(home + "/") === 0))
        return "~" + dir.slice(home.length);
    return dir;
}

// A new capture. Returns the preview state, or null for a path that isn't one (state unchanged).
// `copied` is whether the script already put the image on the clipboard.
function show(path, copied) {
    if (!validPath(path))
        return null;
    return { path: path, copied: copied === true || copied === "copied" || copied === "1" };
}

// The caption's status line.
function status(state, discarded, failed) {
    if (discarded)
        return "Discarded";
    if (failed)
        return "Couldn't copy, is wl-clipboard installed?";
    return state.copied ? "Copied to clipboard" : "Saved";
}

// "c.png in ~/Pictures/Screenshots" (the caption's second line).
function where(p, home) {
    return baseName(p) + " in " + folderLabel(p, home);
}

// The image viewer's command for the capture: open it, or (markup) open it straight in Mark up.
// The path is its own argument, after "--" so a name starting with "-" stays a name.
var VIEWER = "qview";
function viewerCommand(p, markup) {
    return markup ? [VIEWER, "--markup", p] : [VIEWER, "--", p];
}
