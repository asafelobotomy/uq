// A tiny stand-in for document.documentElement: just the calls uq-tokens may make.
export function fakeRoot() {
    const props = new Map(), attrs = new Map(), calls = [];
    return {
        props, attrs, calls,
        style: {
            setProperty(k, v) { calls.push("set"); props.set(k, String(v)); },
            removeProperty(k) { calls.push("remove"); props.delete(k); },
            getPropertyValue(k) { return props.has(k) ? props.get(k) : ""; }
        },
        setAttribute(k, v) { attrs.set(k, String(v)); },
        removeAttribute(k) { attrs.delete(k); },
        getAttribute(k) { return attrs.has(k) ? attrs.get(k) : null; }
    };
}
