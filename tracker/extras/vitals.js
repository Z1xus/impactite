const t = window.impactite;
const url = location.href;
const vitals = {};
const watch = (type, take, options) => {
	try {
		new PerformanceObserver((list) => list.getEntries().forEach(take)).observe({
			type,
			buffered: true,
			...options,
		});
	} catch {}
};

watch("largest-contentful-paint", (entry) => (vitals.lcp = entry.startTime));
watch("paint", (entry) => {
	if (entry.name == "first-contentful-paint") vitals.fcp = entry.startTime;
});
watch(
	"event",
	(entry) => {
		if (entry.interactionId) vitals.inp = Math.max(vitals.inp || 0, entry.duration);
	},
	{ durationThreshold: 16 },
);

// layout shifts add up in windows of at most 5 s with gaps under 1 s, the worst window counts
let shift = 0;
let first = 0;
let last = 0;
watch("layout-shift", (entry) => {
	if (entry.hadRecentInput) return;
	if (entry.startTime - last < 1e3 && entry.startTime - first < 5e3) shift += entry.value;
	else {
		first = entry.startTime;
		shift = entry.value;
	}
	last = entry.startTime;
	vitals.cls = Math.max(vitals.cls || 0, shift);
});

const navigation = performance.getEntriesByType("navigation")[0];
if (navigation) vitals.ttfb = navigation.responseStart;

let sent;
const send = (event) => {
	if (sent || (event.type != "pagehide" && !document.hidden) || !Object.keys(vitals).length)
		return;
	sent = 1;
	t.send("~vitals", { u: url, props: vitals, interactive: false });
};
addEventListener("visibilitychange", send, true);
addEventListener("pagehide", send);
