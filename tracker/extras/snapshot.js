import { snapshot } from "rrweb-snapshot";

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
		maskTextSelector: "[data-impactite-mask]",
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
