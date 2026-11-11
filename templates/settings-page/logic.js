.pragma library
// The page's pure logic: no Qt calls, so it can be tested with Node too
// (docs/settings-pages.md, "Files in the page folder").

/// The status line for `status` (the output of the "status" command), or null while unknown.
function describe(status) {
    if (!status)
        return "Checking…";
    return "Running: " + String(status.state || "unknown");
}

/// The error text a failed result should show, or "" when it worked.
function problemOf(result) {
    return result && result.error ? String(result.error) : "";
}
