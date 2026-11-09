import QtQuick
import QtTest
import "../../shell/logic/sanitize.js" as S
import "../../shell/notif/builtin"

TestCase {
    name: "sanitize"

    // Renders sanitiser output in a real StyledText to check what Qt makes of it.
    Text {
        id: styled
        width: 400
        textFormat: Text.StyledText
    }

    NotifBody {
        id: nb
        width: 200
        raw: '<b>hi</b> <img src="x"> <a href="javascript:x">l</a>'
    }

    // Every tag in `s` is one of the allowed ones, and every open tag is closed in order.
    function wellFormed(s) {
        var re = /<(\/?)([a-z]+)([^>]*)>/g;
        var stack = [];
        var m;
        while ((m = re.exec(s)) !== null) {
            var name = m[2];
            if (["b", "i", "u", "a", "br"].indexOf(name) < 0)
                return "bad tag " + m[0];
            if (name === "br") {
                if (m[1] !== "" || m[3] !== "")
                    return "bad br " + m[0];
                continue;
            }
            if (m[1] === "/") {
                if (m[3] !== "" || stack.pop() !== name)
                    return "misnested at " + m[0];
            } else {
                if (name === "a" ? !/^ href="[^"'<>]*"$/.test(m[3]) : m[3] !== "")
                    return "bad attrs " + m[0];
                stack.push(name);
            }
        }
        if (stack.length > 0)
            return "unclosed " + stack.join(",");
        // No `<` or `>` outside the tags just checked, and every `&` starts an escape.
        var rest = s.replace(re, "");
        if (/[<>]/.test(rest))
            return "stray angle bracket";
        if (/&(?!(amp|lt|gt|quot);)/.test(rest))
            return "unescaped &";
        return "";
    }

    function test_sanitize_allowed() {
        compare(S.sanitize("<b>bold</b> <i>it</i> <u>un</u>"), "<b>bold</b> <i>it</i> <u>un</u>");
        compare(S.sanitize("a<br>b<br/>c<BR />d"), "a<br>b<br>c<br>d");
        compare(S.sanitize("<B>x</B><I>y</i>"), "<b>x</b><i>y</i>");
        compare(S.sanitize('<a href="https://x.org/?a=1">link</a>'),
                '<a href="https://x.org/?a=1">link</a>');
        compare(S.sanitize("<a href='y'>q</a>"), '<a href="y">q</a>');
        compare(S.sanitize("line1\nline2"), "line1<br>line2");
        compare(S.sanitize(""), "");
        compare(S.sanitize(undefined), "");
        compare(S.sanitize(42), "");
    }

    function test_sanitize_strips() {
        compare(S.sanitize('<img src="file:///etc/passwd">x'), "x");
        compare(S.sanitize('<img src="https://evil/t.png"/>'), "");
        compare(S.sanitize('<font color="red" size="40">big</font>'), "big");
        compare(S.sanitize('<span style="font-size:400px">s</span>'), "s");
        compare(S.sanitize("a<script>alert(1)</script>b"), "ab");
        compare(S.sanitize("a<SCRIPT type=x>alert(1)<b>x</b></ScRiPt>b"), "ab");
        compare(S.sanitize("a<style>p{color:red}</style>b"), "ab");
        compare(S.sanitize("a<script>never closed"), "a");
        compare(S.sanitize('<iframe src="https://x">f</iframe>'), "f");
        compare(S.sanitize("<!-- hidden <b> -->t"), "t");
        compare(S.sanitize("<b onclick=\"x()\">c</b>"), "<b>c</b>");
        compare(S.sanitize("<b/>z"), "z");
        compare(wellFormed(S.sanitize('<p><table><tr><td>x</td></tr></table></p><qt><html>y')), "");
    }

    function test_sanitize_unclosed() {
        compare(S.sanitize("<b><i>x"), "<b><i>x</i></b>");
        compare(S.sanitize('<a href="h">x'), '<a href="h">x</a>');
    }

    function test_sanitize_misnested() {
        var out = S.sanitize("<b><i>x</b>y</i>");
        compare(wellFormed(out), "");
        verify(out.indexOf("x") >= 0 && out.indexOf("y") >= 0);
        compare(S.sanitize("</b>x</i>"), "x");
        compare(wellFormed(S.sanitize("<u><a href='q'><b>1</u>2</a>3</b>")), "");
        compare(S.sanitize('<a href="1"><a href="2">x</a></a>'), '<a href="1">x</a>');
    }

    function test_sanitize_entities() {
        compare(S.sanitize("&lt;b&gt;x&lt;/b&gt;"), "&lt;b&gt;x&lt;/b&gt;");
        compare(S.sanitize("&#60;img&#62;"), "&lt;img&gt;");
        compare(S.sanitize("&#x3C;&#X3c;"), "&lt;&lt;");
        compare(S.sanitize("&quot;&apos;&amp;"), "&quot;'&amp;");
        compare(S.sanitize("&foo; &nbsp;"), "&amp;foo; &amp;nbsp;");
        compare(S.sanitize("a & b"), "a &amp; b");
        compare(S.sanitize("&"), "&amp;");
        compare(S.sanitize("&#0;&#xD800;&#99999999;"), "&amp;#0;&amp;#xD800;&amp;#99999999;");
        compare(S.sanitize("1 < 2"), "1 &lt; 2");
        compare(S.sanitize("<"), "&lt;");
        compare(S.sanitize("a <b"), "a &lt;b");
        compare(S.sanitize("x > y \"q\""), "x &gt; y &quot;q&quot;");
        compare(S.sanitize("&#10;"), "<br>");
        compare(S.sanitize("a‮b"), "a b");
    }

    function test_sanitize_href() {
        var js = S.sanitize('<a href="javascript:alert(1)" onclick="evil()">j</a>');
        compare(js, '<a href="javascript:alert(1)">j</a>');
        compare(S.sanitize('<a href="http://x/?a=1&amp;b=2">h</a>'), '<a href="http://x/?a=1&b=2">h</a>');
        compare(S.sanitize('<a href="x\' href=\'javascript:evil">A</a>'),
                '<a href="x%27 href=%27javascript:evil">A</a>');
        compare(S.sanitize("<a href='x\"onmouseover=\"y'>q</a>"), '<a href="x%22onmouseover=%22y">q</a>');
        compare(S.sanitize("<a href=x<y>q</a>"), '<a href="x%3Cy">q</a>');
        compare(S.sanitize("<a href=unq onclick=z>q</a>"), '<a href="unq">q</a>');
        compare(S.sanitize("<a>plain</a>"), "plain");
        compare(S.sanitize('<a name="n" href="">e</a>'), "e");
        compare(S.sanitize('<a HREF="u">c</a>'), '<a href="u">c</a>');
        var long = "";
        for (var i = 0; i < 1000; i++)
            long += "z";
        compare(S.sanitize('<a href="' + long + '">l</a>'), '<a href="' + long.slice(0, 512) + '">l</a>');
        for (var k = 0; k < 6; k++) {
            var o = S.sanitize(['<a href="&quot;><img src=x>">x</a>', "<a href=\"\" onclick=\"x\">y</a>",
                                '<a href="a" href="b">z</a>', "<a\nhref='c'\n>w</a>",
                                "<a href=\"x\" style=\"y\">v</a>", "<a href=\"x>y\">t</a>"][k]);
            compare(wellFormed(o), "", o);
            compare(o.indexOf("onclick"), -1);
            compare(o.indexOf("style"), -1);
        }
    }

    function test_sanitize_huge() {
        var chunk = "<b><i>&amp;x</b><a href='&#60;" + "'>y</a>&#x41;<span>&</span>< <script>z</script>\n";
        var big = "";
        while (big.length < 1024 * 1024)
            big += chunk;
        var t0 = Date.now();
        var out = S.sanitize(big);
        verify(Date.now() - t0 < 2000, "took " + (Date.now() - t0) + " ms");
        verify(out.length < 64 * 1024, "output " + out.length);
        compare(wellFormed(out), "");
        verify(S.plainBody(big).length <= S.MAX_BODY_OUT + 1);

        var plain = "";
        while (plain.length < 1024 * 1024)
            plain += "abcdefgh";
        var p = S.sanitize(plain);
        compare(p.length, S.MAX_BODY_OUT + 1);
        compare(p.slice(-1), "…");

        var lt = "";
        while (lt.length < S.MAX_BODY_IN * 2)
            lt += "<a ";
        verify(S.sanitize(lt).indexOf("&lt;a") === 0);
        var amp = "";
        while (amp.length < S.MAX_BODY_IN * 2)
            amp += "&#";
        verify(S.sanitize(amp).indexOf("&amp;#") === 0);
    }

    function test_plain_body() {
        compare(S.plainBody("<b>Hi</b> <a href='x'>there</a><br>&lt;3 &amp; <script>no</script>"),
                "Hi there\n<3 & ");
        compare(S.plainBody("a\nb"), "a\nb");
        compare(S.plainBody(null), "");
        compare(S.plainBody("&#x41;&bogus;"), "A&bogus;");
    }

    function test_timeout_ms() {
        compare(S.timeoutMs(3000, 2), 0);
        compare(S.timeoutMs(-1, 2), 0);
        compare(S.timeoutMs(0, 1), 0);
        compare(S.timeoutMs(-1, 1), S.DEFAULT_TIMEOUT_MS);
        compare(S.timeoutMs(NaN, 1), S.DEFAULT_TIMEOUT_MS);
        compare(S.timeoutMs("500", 1), S.DEFAULT_TIMEOUT_MS);
        compare(S.timeoutMs(undefined, 0), S.DEFAULT_TIMEOUT_MS);
        compare(S.timeoutMs(Infinity, 1), S.DEFAULT_TIMEOUT_MS);
        compare(S.timeoutMs(10, 1), 1000);
        compare(S.timeoutMs(2500, 0), 2500);
        compare(S.timeoutMs(999999, 1), S.MAX_TIMEOUT_MS);
        compare(S.DEFAULT_TIMEOUT_MS, 5000);
        compare(S.MAX_TIMEOUT_MS, 60000);
    }

    function test_popup_allowed() {
        verify(S.popupAllowed(false, 0));
        verify(S.popupAllowed(false, 1));
        verify(!S.popupAllowed(true, 0));
        verify(!S.popupAllowed(true, 1));
        verify(S.popupAllowed(true, 2));
    }

    function test_image_ref() {
        compare(S.imageRef("image://qsimage/123/0"), "image://qsimage/123/0");
        compare(S.imageRef("/home/u/a b#c.png"), "file:///home/u/a%20b%23c.png");
        compare(S.imageRef("file:///tmp/x.png"), "file:///tmp/x.png");
        compare(S.imageRef("file://host/x.png"), "");
        compare(S.imageRef("//host/x.png"), "");
        compare(S.imageRef("http://x/y.png"), "");
        compare(S.imageRef("https://x/y.png"), "");
        compare(S.imageRef("data:image/png;base64,AAAA"), "");
        compare(S.imageRef("rel/x.png"), "");
        compare(S.imageRef("dialog-information"), "");
        compare(S.imageRef(""), "");
        compare(S.imageRef(null), "");
    }

    function test_popup_view_sharing() {
        var long = "";
        for (var i = 0; i < 300; i++)
            long += "s";
        var n = {
            appName: "App\u0000Name", appIcon: "firefox", summary: long, body: "<b>secret</b>",
            urgency: 1, image: "",
            actions: [{ identifier: "default", text: "Open" }, { identifier: "a1", text: "One\n" },
                      { identifier: "a2", text: "Two" }, { identifier: "a3", text: "Three" },
                      { identifier: "a4", text: "Four" }]
        };
        var v = S.popupView(n, false);
        compare(v.appName, "App Name");
        compare(v.appIcon, "firefox");
        compare(v.summary.length, 200);
        compare(v.body, "<b>secret</b>");
        compare(v.actions.length, S.MAX_ACTIONS);
        compare(v.actions[0].identifier, "a1");
        compare(v.actions[0].text, "One");
        compare(v.actions[2].identifier, "a3");

        var s = S.popupView(n, true);
        compare(s.appName, "App Name");
        compare(s.appIcon, "firefox");
        compare(s.summary, "");
        compare(s.body, "");
        compare(s.actions.length, 0);

        var e = S.popupView({}, false);
        compare(e.appName, "");
        compare(e.appIcon, "");
        compare(e.body, "");
        compare(e.actions.length, 0);
        var lbl = "";
        for (var k = 0; k < 60; k++)
            lbl += "x";
        compare(S.popupView({ actions: [{ identifier: "z", text: lbl }] }, false).actions[0].text.length,
                S.MAX_ACTION_TEXT);
    }

    function test_store_caps() {
        compare(S.MAX_KEEP, 50);
        compare(S.MAX_POPUPS, 3);
        compare(S.MAX_ACTIONS, 3);
        compare(S.MAX_ACTION_TEXT, 40);
        compare(S.MAX_BODY_IN, 8192);
        compare(S.MAX_BODY_OUT, 2000);
        compare(S.trimKept(0), 0);
        compare(S.trimKept(50), 0);
        compare(S.trimKept(51), 1);
        compare(S.trimKept(75), 25);
        compare(S.trimKept(-3), 0);
    }

    function test_notif_body() {
        var t = nb.children[0];
        compare(t.textFormat, Text.StyledText);
        compare(t.text, S.sanitize(nb.raw));
        compare(t.Accessible.name, "hi  l");
        compare(nb.hoveredLink, "");
        t.linkHovered("javascript:x");
        compare(nb.hoveredLink, "javascript:x");
        compare(nb.children[1].text, "javascript:x");
        t.linkActivated("javascript:x");
        t.linkHovered("");
        compare(nb.hoveredLink, "");
        compare(nb.children[1].text, "");
    }

    function linkOf(raw) {
        styled.text = S.sanitize(raw);
        return styled.linkAt(2, styled.height / 2);
    }

    function test_href_in_styled_text() {
        compare(linkOf('<a href="x\' href=\'javascript:evil">AAAAAAAA</a>'), "x%27 href=%27javascript:evil");
        compare(linkOf("<a href='x\"onclick=\"y'>AAAAAAAA</a>"), "x%22onclick=%22y");
        compare(linkOf('<a href="http://x/?a=1&amp;b=2&c=3">AAAAAAAA</a>'), "http://x/?a=1&b=2&c=3");
        compare(linkOf('<a href="a&lt;b&gt;c">AAAAAAAA</a>'), "a%3Cb%3Ec");
        compare(linkOf("<a href=plain>AAAAAAAA</a>"), "plain");
        compare(linkOf("<a>AAAAAAAA</a>"), "");
    }

    function test_depth_cap() {
        var deep = "";
        while (deep.length < 4096)
            deep += "<b>";
        var closes = "";
        while (closes.length < 4096)
            closes += "</i>";
        var t0 = Date.now();
        var out = S.sanitize(deep + "x" + closes);
        verify(Date.now() - t0 < 500, "took " + (Date.now() - t0) + " ms");
        compare(wellFormed(out), "");
        compare(out.split("<b>").length - 1, 16);
        var mixed = "";
        for (var i = 0; i < 20; i++)
            mixed += "<i>";
        compare(S.sanitize(mixed + '<a href="h">x</a>').indexOf("<a"), -1);
        compare(S.sanitize('<a href="1">x</a><a href="2">y</a>'), '<a href="1">x</a><a href="2">y</a>');
    }

    function test_truncate_surrogate() {
        var a = "";
        for (var i = 0; i < 1999; i++)
            a += "a";
        var out = S.sanitize(a + "\ud83d\ude00tail");
        compare(out, a + "…");
        compare(S.plainBody(a + "\ud83d\ude00"), a + "…");
        var b = a.slice(1);
        compare(S.sanitize(b + "\ud83d\ude00z"), b + "\ud83d\ude00…");
    }
}
