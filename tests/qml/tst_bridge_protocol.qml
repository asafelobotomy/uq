import QtQuick
import QtTest
import "../../shell/core"

TestCase {
    name: "bridge-protocol"

    property var sent: []
    SignalSpy { id: themeSpy; target: Bridge; signalName: "themeChanged" }
    SignalSpy { id: layoutSpy; target: Bridge; signalName: "layoutChanged" }
    SignalSpy { id: eventSpy; target: Bridge; signalName: "event" }
    SignalSpy { id: resultsSpy; target: Bridge; signalName: "searchResults" }

    function init() {
        sent = [];
        Bridge.reset();
        Bridge.writer = function (s) { sent.push(s); };
        Bridge.processStarted();
        themeSpy.clear(); layoutSpy.clear(); eventSpy.clear(); resultsSpy.clear();
    }

    function test_hello_sets_state() {
        verify(!Bridge.connected);
        Bridge.feed('{"v":1,"event":"Hello","data":{"v":1,"uqd_up":true,"locked":false}}');
        verify(Bridge.connected);
        verify(Bridge.uqdUp);
    }

    function test_call_resolves_and_rejects() {
        var got = null, err = null;
        Bridge.call("Ping", []).then(function (v) { got = v; });
        var req = JSON.parse(sent[0]);
        compare(req.v, 1); compare(req.call, "Ping");
        Bridge.feed(JSON.stringify({ v: 1, id: req.id, ok: "pong" }));
        compare(got, "pong");

        Bridge.call("Bad", []).catch(function (e) { err = e; });
        var req2 = JSON.parse(sent[1]);
        Bridge.feed(JSON.stringify({ v: 1, id: req2.id, err: "nope" }));
        compare(err, "nope");
    }

    function test_pending_rejected_on_exit() {
        var err = null;
        Bridge.call("Slow", []).catch(function (e) { err = e; });
        Bridge.processExited();
        verify(err !== null);
        verify(!Bridge.connected);
        compare(Object.keys(Bridge.pending).length, 0);
    }

    function test_events_and_properties() {
        Bridge.feed('{"v":1,"event":"ThemeChanged","data":{"name":"a","variant":"dark","tokens_path":"/x"}}');
        compare(themeSpy.count, 1);
        Bridge.feed('{"v":1,"event":"LayoutChanged","data":{"workspace":2,"layout":"dwindle"}}');
        compare(layoutSpy.count, 1);
        compare(layoutSpy.signalArguments[0][0], 2);
        compare(layoutSpy.signalArguments[0][1], "dwindle");
        compare(Bridge.weather, null);
        Bridge.feed('{"v":1,"event":"Weather","data":{"temp_c":3,"code":1,"is_day":true,"location_name":"Oslo"}}');
        compare(Bridge.weather.location_name, "Oslo");
        Bridge.feed('{"v":1,"event":"Weather","data":null}');
        compare(Bridge.weather, null);
        Bridge.feed('{"v":1,"event":"PublicIp","data":{"country_code":"NO","country":"Norway"}}');
        compare(Bridge.publicIp.country_code, "NO");
        Bridge.feed('{"v":1,"event":"UqdDown","data":{}}');
        verify(!Bridge.uqdUp);
        Bridge.feed('{"v":1,"event":"UqdUp","data":{}}');
        verify(Bridge.uqdUp);
        verify(eventSpy.count >= 6);
    }

    function test_search_results_signal() {
        Bridge.feed('{"v":1,"event":"SearchResults","data":{"generation":3,"provider":"calc","provider_name":"Calculator","source_dir":"builtin","items":[{"id":"calc","title":"2"}]}}');
        compare(resultsSpy.count, 1);
        compare(resultsSpy.signalArguments[0][0].generation, 3);
        compare(resultsSpy.signalArguments[0][0].items[0].title, "2");
        Bridge.feed('{"v":1,"event":"SearchResults","data":null}');
        compare(resultsSpy.count, 1);
    }

    function test_junk_ignored() {
        Bridge.feed("garbage");
        Bridge.feed('{"v":2,"event":"ThemeChanged"}');
        Bridge.feed('{"v":1,"id":999,"ok":1}');
        compare(themeSpy.count, 0);
    }

    function test_sharing_from_screencast() {
        verify(!Bridge.sharing);
        Bridge.handleRawEvent("screencast", "1,0");
        verify(Bridge.sharing);
        Bridge.handleRawEvent("screencast", "0,0");
        verify(!Bridge.sharing);
        Bridge.handleRawEvent("other", "1,0");
        verify(!Bridge.sharing);
    }

    function test_restart_backoff_and_stable_reset() {
        Bridge.stableMs = 50;
        Bridge.processExited();
        compare(Bridge.attempt, 1);
        compare(Bridge.restartDelay, 1000);
        Bridge.processStarted();
        Bridge.processExited();
        compare(Bridge.attempt, 2);
        compare(Bridge.restartDelay, 2000);
        Bridge.processStarted();
        wait(150);
        compare(Bridge.attempt, 0);
    }

    function test_double_exit_counts_once() {
        Bridge.processExited();
        Bridge.processExited();
        compare(Bridge.attempt, 1);
    }
}
