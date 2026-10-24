import { snapshot } from "rrweb-snapshot";

const hide = (text, element) =>
	element?.closest("[data-impactite-unmask]") ? text : text.replace(/\S/g, "*");

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
