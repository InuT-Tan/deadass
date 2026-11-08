// deadass soul-pickup watcher, appended after Lovelock's HUD script in the play VPK.
//
// Every soul pickup pops a popup instance into #HudEventIndicatorsPanel. Observed
// classes: "gold" for breakable boxes (95 souls each, 190 when two merge) and
// "gold_small" for trooper last hits (23-24). Each new instance is reported once as
//   [DEADASS]{"schema":1,"event":"soul_pickup","size":"large"|"small",...}
// Lovelock only logs these as a diagnostic capped at 60 per session.
(function () {
    "use strict";

    var LOG_PREFIX = "[DEADASS]";
    var POLL_INTERVAL_SECONDS = 0.1;
    var CONTAINER_ID = "HudEventIndicatorsPanel";
    var SIZES = [
        { cls: "gold", size: "large" },
        { cls: "gold_small", size: "small" }
    ];
    var context = $.GetContextPanel();
    var sessionId = Date.now().toString(36) + "-souls";
    var sequence = 0;
    var credited = [];
    var container = null;

    function valid(panel) {
        try {
            return !!(panel && panel.IsValid && panel.IsValid());
        } catch (_error) {
            return false;
        }
    }

    function hudRoot() {
        var panel = context;
        for (var i = 0; i < 64 && valid(panel); i++) {
            var parent = panel.GetParent ? panel.GetParent() : null;
            if (!valid(parent)) {
                break;
            }
            panel = parent;
        }
        return panel;
    }

    function findContainer() {
        if (valid(container)) {
            return container;
        }
        try {
            var root = hudRoot();
            container = valid(root) && root.FindChildTraverse ? root.FindChildTraverse(CONTAINER_ID) : null;
        } catch (_error) {
            container = null;
        }
        return container;
    }

    function withClass(panel, className) {
        try {
            return panel.FindChildrenWithClassTraverse ? panel.FindChildrenWithClassTraverse(className) : [];
        } catch (_error) {
            return [];
        }
    }

    // The popup text animates ("", "+", then the number), so the amount is best-effort.
    function amountOf(panel) {
        var labels = withClass(panel, "HudIndicatorText");
        var text = labels.length > 0 && valid(labels[0]) ? String(labels[0].text || "") : "";
        var value = parseInt(text.replace(/[^0-9]/g, ""), 10);
        return isNaN(value) ? null : value;
    }

    function emit(size, amount) {
        sequence++;
        var payload = {
            schema: 1,
            event: "soul_pickup",
            mod_version: "0.1.0",
            session_id: sessionId,
            sequence: sequence,
            client_time_ms: Date.now(),
            size: size
        };
        if (amount !== null) {
            payload.amount = amount;
        }
        $.Msg(LOG_PREFIX + JSON.stringify(payload));
    }

    function poll() {
        if (!valid(context)) {
            return;
        }
        var kept = [];
        for (var k = 0; k < credited.length; k++) {
            if (valid(credited[k])) {
                kept.push(credited[k]);
            }
        }
        credited = kept;

        var host = findContainer();
        if (valid(host)) {
            for (var s = 0; s < SIZES.length; s++) {
                var hits = withClass(host, SIZES[s].cls);
                for (var h = 0; h < hits.length; h++) {
                    if (!valid(hits[h]) || credited.indexOf(hits[h]) !== -1) {
                        continue;
                    }
                    credited.push(hits[h]);
                    emit(SIZES[s].size, amountOf(hits[h]));
                }
            }
        }
        $.Schedule(POLL_INTERVAL_SECONDS, poll);
    }

    $.Schedule(POLL_INTERVAL_SECONDS, poll);
})();
