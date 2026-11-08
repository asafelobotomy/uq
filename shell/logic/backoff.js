.pragma library

// Bridge restart delays: 1, 2, 4, 8, 16, 30, 30... seconds.
var STEPS = [1000, 2000, 4000, 8000, 16000, 30000];

function next(attempt) {
    var i = Math.floor(Number(attempt));
    if (!(i >= 0))
        i = 0;
    return i < STEPS.length ? STEPS[i] : STEPS[STEPS.length - 1];
}
