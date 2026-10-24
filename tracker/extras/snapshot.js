import { snapshot } from "rrweb-snapshot";

// embeds keep their text in shadow roots, so the search climbs out through each host
const shown = (element) => {
	for (; element; element = element.parentElement ?? element.getRootNode().host)
		if (element.hasAttribute("data-impactite-unmask")) return true;
	return false;
};
const hide = (text, element) => (shown(element) ? text : text.replace(/\S/g, "*"));
const hideTyped = (text, element) =>
	shown(element) && !element.matches("[type=password], [data-rr-is-password]")
		? text
		: "*".repeat(text.length);

const t = window.impactite;
const src = document.currentScript.src;
const root = document.documentElement;
const body = JSON.stringify({
	d: t.domain.split(",")[0],
	...t.shot,
	width: root.clientWidth,
	height: root.scrollHeight,
	node: snapshot(document, {
		maskAllInputs: true,
		maskTextSelector: "*",
		maskTextFn: hide,
		maskInputFn: hideTyped,
		blockSelector: "[data-impactite-block]",
		inlineStylesheet: true,
	}),
});
new Response(new Blob([body]).stream().pipeThrough(new CompressionStream("gzip")))
	.blob()
	.then((packed) =>
		fetch(new URL("/api/snapshot", src), {
			method: "POST",
			body: packed,
		}),
	)
	.catch(() => {});
