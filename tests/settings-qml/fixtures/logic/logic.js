.pragma library
// A page's pure logic in a sibling file (docs/settings-pages.md, "Files in the page folder").

function describe(status) {
    if (!status)
        return "Checking…";
    return status.running ? "Running" : "Stopped";
}
