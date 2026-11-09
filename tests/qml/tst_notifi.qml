import QtQuick
import QtTest
import "../../shell/logic/notifi.js" as N

TestCase {
    name: "notifi"

    readonly property string hello: '{"event":"hello","name":"notifd","protocol":1,"daemon":"0.18.0","pid":12,"seq":0}'

    function fresh() {
        return { greeted: false, pending: {} };
    }

    function greeted(pending) {
        var r = N.handleLine(fresh(), hello);
        compare(r.effect.kind, "hello");
        r.state.pending = pending || {};
        return r.state;
    }

    // Review Focus 4: a non-hello first line, an oversized line, a non-JSON line or a reply with
    // an unknown id must never act.
    function test_notifi_handle_line() {
        var r = N.handleLine(fresh(), hello);
        compare(r.effect.kind, "hello");
        compare(r.state.greeted, true);

        // Wrong name, wrong protocol, missing name, non-JSON: not noti-fi.
        compare(N.handleLine(fresh(), '{"event":"hello","name":"other","protocol":1}').effect.kind, "unavailable");
        compare(N.handleLine(fresh(), '{"event":"hello","name":"notifd","protocol":2}').effect.kind, "unavailable");
        compare(N.handleLine(fresh(), '{"event":"hello","name":"notifd","protocol":"1"}').effect.kind, "unavailable");
        compare(N.handleLine(fresh(), '{"event":"hello","protocol":1}').effect.kind, "unavailable");
        compare(N.handleLine(fresh(), 'not json').effect.kind, "unavailable");
        compare(N.handleLine(fresh(), 'null').effect.kind, "unavailable");
        compare(N.handleLine(fresh(), '[1]').effect.kind, "unavailable");
        // Any other line before the hello.
        compare(N.handleLine(fresh(), '{"event":"state","dnd":true}').effect.kind, "unavailable");
        compare(N.handleLine(fresh(), '{"id":1,"result":{}}').effect.kind, "unavailable");
        compare(N.handleLine(fresh(), '{"event":"error","code":"too_many_connections"}').effect.kind, "unavailable");
        compare(N.handleLine(fresh(), "").effect.kind, "unavailable");

        // Reply for a pending id: resolved and removed.
        var s = greeted({ "3": "osd", "4": "dnd" });
        r = N.handleLine(s, '{"id":3,"result":{"ok":true}}');
        compare(r.effect.kind, "reply");
        compare(r.effect.id, 3);
        compare(r.effect.method, "osd");
        compare(r.effect.ok, true);
        compare(r.effect.value.ok, true);
        verify(!r.state.pending.hasOwnProperty("3"));
        verify(r.state.pending.hasOwnProperty("4"));
        verify(s.pending.hasOwnProperty("3"), "input state not mutated");

        // Error reply (noti-fi's object form, and a bare string code).
        r = N.handleLine(r.state, '{"id":4,"error":{"code":"bad_params","message":"x"}}');
        compare(r.effect.kind, "reply");
        compare(r.effect.ok, false);
        compare(r.effect.error, "bad_params");
        verify(!r.state.pending.hasOwnProperty("4"));
        r = N.handleLine(greeted({ "5": "osd" }), '{"id":5,"error":"bad_params"}');
        compare(r.effect.ok, false);
        compare(r.effect.error, "bad_params");

        // The same id twice: the second is unknown.
        s = greeted({ "7": "osd" });
        r = N.handleLine(s, '{"id":7,"result":{}}');
        compare(r.effect.kind, "reply");
        compare(N.handleLine(r.state, '{"id":7,"result":{}}').effect.kind, "ignore");

        // Unknown id, id:null error, prototype-ish ids.
        s = greeted({ "3": "osd" });
        compare(N.handleLine(s, '{"id":99,"result":{"ok":true}}').effect.kind, "ignore");
        compare(N.handleLine(s, '{"id":null,"error":{"code":"bad_request"}}').effect.kind, "ignore");
        compare(N.handleLine(s, '{"id":"toString","result":{}}').effect.kind, "ignore");
        compare(N.handleLine(s, '{"id":"__proto__","result":{}}').effect.kind, "ignore");

        // Oversized (70,000 chars, and a multibyte line over 64 KiB in bytes but not in chars).
        var big = '{"id":3,"result":{"pad":"' + "x".repeat(70000) + '"}}';
        compare(N.handleLine(s, big).effect.kind, "ignore");
        var wide = '{"event":"state","dnd":true,"p":"' + "é".repeat(33000) + '"}';
        verify(wide.length < N.MAX_LINE);
        compare(N.handleLine(s, wide).effect.kind, "ignore");
        // Oversized first line is not a hello.
        compare(N.handleLine(fresh(), big).effect.kind, "unavailable");

        // Non-JSON and junk after the hello.
        compare(N.handleLine(s, "garbage").effect.kind, "ignore");
        compare(N.handleLine(s, "").effect.kind, "ignore");
        compare(N.handleLine(s, "42").effect.kind, "ignore");
        compare(N.handleLine(s, hello).effect.kind, "ignore");
        compare(N.handleLine(s, '{"event":"notification","id":1}').effect.kind, "ignore");

        // State events: dnd taken strictly as a bool.
        r = N.handleLine(s, '{"event":"state","seq":4,"t":1,"dnd":true,"unread":2}');
        compare(r.effect.kind, "state");
        compare(r.effect.dnd, true);
        compare(N.handleLine(s, '{"event":"state","dnd":false}').effect.dnd, false);
        compare(N.handleLine(s, '{"event":"state","dnd":"yes"}').effect.dnd, false);
        compare(N.handleLine(s, '{"event":"state"}').effect.dnd, false);

        // Bad state input.
        compare(N.handleLine(null, hello).effect.kind, "hello");
        compare(N.handleLine(undefined, 'x').effect.kind, "unavailable");
    }

    function test_request_line() {
        var line = N.request(3, "osd", { kind: "volume" });
        verify(line.indexOf("\n") < 0);
        var m = JSON.parse(line);
        compare(m.id, 3);
        compare(m.method, "osd");
        compare(m.params.kind, "volume");
        compare(Object.keys(m).length, 3);
        compare(JSON.parse(N.request(4, "subscribe")).params, {});
        // A newline in a param stays escaped inside the one line.
        verify(N.request(5, "x", { s: "a\nb" }).indexOf("\n") < 0);
    }

    function test_osd_params() {
        compare(N.osdParams("volume", 55.4, undefined, false),
                { kind: "volume", value: 55, max: 100, muted: false });
        compare(N.osdParams("volume", 140, 100, false).value, 100);
        compare(N.osdParams("volume", -5, 100, false).value, 0);
        compare(N.osdParams("brightness", 300, 937, false),
                { kind: "brightness", value: 300, max: 937, muted: false });
        compare(N.osdParams("mic", 0, 100, true).muted, true);
        // muted is optional in noti-fi's protocol: undefined means false; a non-bool is invalid.
        compare(N.osdParams("brightness", 40, 100).muted, false);
        compare(N.osdParams("volume", 40, 100, "yes"), null);
        compare(N.osdParams("speaker", 10, 100, false), null);
        compare(N.osdParams("mic", 1, 0, true), null);
        compare(N.osdParams("mic", 1, 1.5, true), null);
        compare(N.osdParams("mic", 1, -1, true), null);
        compare(N.osdParams("volume", NaN, 100, false), null);
        compare(N.osdParams("volume", Infinity, 100, false), null);
        compare(N.osdParams("volume", "50", 100, false), null);
        compare(N.osdParams("volume", 50, 100, "true"), null);
        compare(N.osdParams("volume", 50, 100, undefined), null);
    }

    // DND from a `subscribe` snapshot ({status, seq}) or a `dnd` reply (Status).
    function test_reply_dnd() {
        compare(N.replyDnd("subscribe", { status: { dnd: true }, seq: 3 }), true);
        compare(N.replyDnd("subscribe", { status: { dnd: false }, seq: 3 }), false);
        compare(N.replyDnd("dnd", { dnd: true }), true);
        compare(N.replyDnd("dnd", { dnd: "on" }), null);
        compare(N.replyDnd("subscribe", null), null);
        compare(N.replyDnd("osd", { dnd: true }), null);
    }
}
