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
        compare(r.effect.status.dnd, true);
        compare(r.effect.status.unread, 2);
        compare(N.handleLine(s, '{"event":"state","dnd":false}').effect.status.dnd, false);
        compare(N.handleLine(s, '{"event":"state","dnd":"yes"}').effect.status.dnd, false);
        compare(N.handleLine(s, '{"event":"state"}').effect.status.dnd, false);

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
    }

    function test_status_fields() {
        var full = N.statusFields({ dnd: true, unread: 3, center: true, sharing: true, holding: "dnd",
            latest: { id: 9, t: 1, appName: "Mail", appIcon: "mail-unread", summary: "Hi", read: false } });
        compare(full, { unread: 3, dnd: true, centre: true, sharing: true, holding: "dnd",
                        latest: { appName: "Mail", appIcon: "mail-unread", summary: "Hi" } });
        // Missing fields.
        compare(N.statusFields({}), { unread: 0, dnd: false, centre: false, sharing: false,
                                      holding: "", latest: null });
        // Wrong types.
        var bad = N.statusFields({ dnd: "yes", unread: "5", center: 1, sharing: "true", holding: 4,
                                   latest: "x" });
        compare(bad, { unread: 0, dnd: false, centre: false, sharing: false, holding: "", latest: null });
        compare(N.statusFields({ latest: [1] }).latest, null);
        compare(N.statusFields({ latest: { appName: 1, appIcon: null, summary: {} } }).latest,
                { appName: "", appIcon: "", summary: "" });
        // Unread: non-negative integer, else 0.
        compare(N.statusFields({ unread: -2 }).unread, 0);
        compare(N.statusFields({ unread: 2.5 }).unread, 0);
        compare(N.statusFields({ unread: NaN }).unread, 0);
        compare(N.statusFields({ unread: 0 }).unread, 0);
        // Text caps.
        var l = N.statusFields({ latest: { appName: "a".repeat(500), appIcon: "i".repeat(2000),
                                           summary: "s".repeat(10240) } }).latest;
        compare(l.appName.length, 64);
        compare(l.appIcon.length, 512);
        compare(l.summary.length, 200);
        // Holding: the six values only.
        var hs = ["", "dnd", "inhibited", "screen sharing", "fullscreen", "focus"];
        for (var i = 0; i < hs.length; i++)
            compare(N.statusFields({ holding: hs[i] }).holding, hs[i]);
        compare(N.statusFields({ holding: "party" }).holding, "");
        compare(N.statusFields({ holding: "toString" }).holding, "");
        // Not an object.
        compare(N.statusFields(null), null);
        compare(N.statusFields("x"), null);
        compare(N.statusFields([1]), null);
        compare(N.statusFields(undefined), null);
    }

    function test_state_effect_carries_status() {
        var s = greeted();
        var r = N.handleLine(s, '{"event":"state","seq":2,"dnd":true,"unread":4,"center":true,'
            + '"holding":"focus","latest":{"appName":"A","appIcon":"i","summary":"S"}}');
        compare(r.effect.kind, "state");
        compare(r.effect.status.unread, 4);
        compare(r.effect.status.centre, true);
        compare(r.effect.status.holding, "focus");
        compare(r.effect.status.latest.summary, "S");
        verify(!r.effect.hasOwnProperty("dnd"));
    }

    // Status from a `subscribe` snapshot ({status, seq}), a `dnd` or a `center` reply (Status).
    function test_reply_status() {
        compare(N.replyStatus("subscribe", { status: { dnd: true, unread: 2 }, seq: 3 }).unread, 2);
        compare(N.replyStatus("subscribe", { status: { dnd: true }, seq: 3 }).dnd, true);
        compare(N.replyStatus("dnd", { dnd: true }).dnd, true);
        compare(N.replyStatus("center", { center: true }).centre, true);
        compare(N.replyStatus("subscribe", null), null);
        compare(N.replyStatus("subscribe", { seq: 1 }), null);
        compare(N.replyStatus("subscribe", { status: 5 }), null);
        compare(N.replyStatus("dnd", "x"), null);
        compare(N.replyStatus("osd", { dnd: true }), null);
        compare(N.replyStatus("status", { dnd: true }), null);
    }
    function test_notifi_expired_ids() {
        compare(N.REQUEST_TIMEOUT_MS, 1000);
        var d = { "1": 1000, "2": 2000, "3": 1500 };
        compare(N.expiredIds(d, 999), []);
        compare(N.expiredIds(d, 1000), ["1"]);
        compare(N.expiredIds(d, 1600).sort(), ["1", "3"]);
        compare(N.expiredIds(d, 5000).sort(), ["1", "2", "3"]);
        compare(N.expiredIds({}, 5000), []);
        compare(N.expiredIds(null, 5000), []);
        // A bad deadline expires at once; a NaN clock expires nothing.
        compare(N.expiredIds({ "4": NaN }, 5000), ["4"]);
        compare(N.expiredIds(d, NaN), []);
    }
}
