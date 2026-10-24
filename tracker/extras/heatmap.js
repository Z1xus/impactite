const t = window.impactite;
const page = t.key();
const width = innerWidth;
const screen = width >= 1024 ? "desktop" : width >= 768 ? "tablet" : "mobile";
let clicks = 0;

addEventListener(
	"click",
	(event) => {
		if (t.key() != page || ++clicks > 50) return;
		t.send("~click", {
			props: {
				x: Math.round(event.clientX - document.documentElement.clientWidth / 2),
				y: Math.round(event.pageY),
				w: innerWidth,
			},
			interactive: false,
		});
	},
	true,
);

const src = document.currentScript.src;
const ask = () =>
	fetch(
		new URL(
			`/api/snapshot?${new URLSearchParams({ d: t.domain.split(",")[0], p: page, w: screen })}`,
			src,
		),
	)
		.then((response) => response.text())
		.then((wanted) => {
			if (wanted != "1" || t.key() != page) return;
			t.shot = { p: page, w: screen };
			document.head.append(
				Object.assign(document.createElement("script"), { src: new URL("snapshot.js", src) }),
			);
		});
// the page settles first, so the snapshot shows it as visitors see it
setTimeout(() => ask().catch(() => {}), 3e3);
