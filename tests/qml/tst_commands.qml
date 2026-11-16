import QtQuick
import QtTest
import "../../shell/logic/commands.js" as Commands
import "../../shell/logic/launcher.js" as L
import "../../shell/logic/rank.js" as Rank

TestCase {
    name: "commands"

    property var profiles: [{ id: "work", name: "Work", active: false },
                            { id: "game", name: "Game", active: true }]

    function ids(rows) { return rows.map(function (r) { return r.id; }); }

    function test_short_queries_offer_nothing() {
        compare(Commands.search("lo", profiles).length, 0);
        compare(Commands.search("", profiles).length, 0);
    }

    function test_power_commands_match_by_word_start() {
        compare(ids(Commands.search("loc", [])), ["power:lock"]);
        compare(ids(Commands.search("shut", [])), ["power:poweroff"]);
        compare(ids(Commands.search("log", [])), ["power:logout"]);
        compare(ids(Commands.search("sleep", [])), ["power:suspend"]);
        compare(Commands.search("ock", []).length, 0);
    }

    function test_profiles() {
        compare(ids(Commands.search("work", profiles)), ["profile:work"]);
        // The active profile has no switch, but Back to normal appears.
        compare(ids(Commands.search("game", profiles)), []);
        verify(ids(Commands.search("profile", profiles)).indexOf("profile-off") >= 0);
        compare(ids(Commands.search("profile off", [{ id: "w", name: "W", active: false }])), []);
    }

    function test_activation_table() {
        compare(JSON.stringify(Commands.activation("power:lock")), JSON.stringify({ call: "Power", args: ["lock"] }));
        compare(JSON.stringify(Commands.activation("profile:work")), JSON.stringify({ call: "ApplyProfile", args: ["work"] }));
        compare(JSON.stringify(Commands.activation("profile-off")), JSON.stringify({ call: "ProfileOff", args: [] }));
        compare(Commands.activation("power:format-disk"), null);
        compare(Commands.activation("profile:"), null);
    }

    function test_launcher_routes_commands() {
        var a = L.activation("commands", { id: "power:suspend" });
        compare(a.kind, "command");
        compare(a.call, "Power");
        compare(JSON.stringify(a.args), JSON.stringify(["suspend"]));
        compare(L.activation("commands", { id: "nope" }), null);
    }

    function test_enter_runs_in_a_terminal_shift_enter_in_the_background() {
        var item = { id: "run-4", actions: ["terminal", "run"] };
        compare(JSON.stringify(L.activation("run", item).args), JSON.stringify([{ run_id: "run-4", terminal: true }]));
        compare(JSON.stringify(L.activation("run", item, true).args), JSON.stringify([{ run_id: "run-4" }]));
        compare(JSON.stringify(L.activation("run", { id: "run-4" }, true).args), JSON.stringify([{ run_id: "run-4" }]));
    }

    function test_shift_enter_takes_a_providers_second_action() {
        var item = { id: "x", actions: ["copy", "type"] };
        compare(L.activation("org.x.Emoji", item).args[2], "copy");
        compare(L.activation("org.x.Emoji", item, true).args[2], "type");
        compare(L.activation("org.x.Emoji", { id: "x", actions: ["copy"] }, true).args[2], "copy");
    }

    function test_noTop_commands_never_lead() {
        var pg = { commands: { provider: "commands", provider_name: "Commands", generation: 1,
                               items: [{ id: "power:poweroff", score: 0.8, noTop: true }] } };
        compare(Rank.merge([], pg, 1).topHit, null);
        pg.commands.items.push({ id: "power:lock", score: 0.5, noTop: false });
        compare(Rank.merge([], pg, 1).topHit.item.id, "power:lock");
    }
}
