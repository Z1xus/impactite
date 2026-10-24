import { record } from "@rrweb/record";

const hide = (text, element) =>
	element?.closest("[data-impactite-unmask]") ? text : text.replace(/\S/g, "*");
const hideTyped = (text, element) =>
	element?.closest("[data-impactite-unmask]") && !element.matches("[type=password], [data-rr-is-password]")
		? text
		: "*".repeat(text.length);

const t = window.impactite;
const src = document.currentScript.src;
const KEY = "impactite_replay";

// one recording per tab, picked up again on the next page until it idles for 30 minutes
let state;
try {
	state = JSON.parse(sessionStorage[KEY]);
} catch {}
if (!state || Date.now() - state.at > 18e5)
	state = { id: crypto.randomUUID().replaceAll("-", ""), at: Date.now() };

let events = [];
const flush = () => {
	if (!events.length) return;
	const body = JSON.stringify({ d: t.domain.split(",")[0], id: state.id, e: events, u: location.href });
	events = [];
	state.at = Date.now();
	try {
		sessionStorage[KEY] = JSON.stringify(state);
	} catch {}
	const url = new URL("/api/replay", src);
	// a page going away can't wait for compression
	if (document.hidden && navigator.sendBeacon(url, body)) return;
	new Response(new Blob([body]).stream().pipeThrough(new CompressionStream("gzip")))
		.blob()
		.then((packed) => fetch(url, { method: "POST", body: packed, keepalive: packed.size < 6e4 }))
		.catch(() => {});
};

record({
	emit: (event) => events.push(event),
	maskAllInputs: true,
	maskTextSelector: "*",
	maskTextFn: hide,
	maskInputFn: hideTyped,
	blockSelector: "[data-impactite-block]",
	sampling: { mousemove: 50, scroll: 150, input: "last" },
	slimDOMOptions: "all",
});
setInterval(flush, 5e3);
addEventListener("visibilitychange", () => document.hidden && flush());
addEventListener("pagehide", flush);
