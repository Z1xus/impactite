const t = window.impactite;
const page = t.key();
const width = innerWidth;
const screen = width >= 1024 ? "desktop" : width >= 768 ? "tablet" : "mobile";
let clicks = 0;

// a path the dashboard can find again in the snapshot
const path = (node) => {
	const parts = [];
	for (; node && node != document.body; node = node.parentElement) {
		if (/^[a-z][\w-]*$/i.test(node.id)) return ["#" + node.id, ...parts].join(">");
		let n = 1;
		for (let sibling = node; (sibling = sibling.previousElementSibling); )
			n += sibling.localName == node.localName;
		parts.unshift(`${node.localName}:nth-of-type(${n})`);
	}
	return ["body", ...parts].join(">");
};

addEventListener(
	"click",
	(event) => {
		if (t.key() != page || ++clicks > 50) return;
		const target = event.target.closest("a,button,input,select,textarea,label,summary,[role=button]") ?? event.target;
		const box = target.getBoundingClientRect();
		// the spot inside the element in thousandths, so layouts at other sizes still line up
		const share = (at, start, size) => (size ? Math.round(((at - start) / size) * 1000) : 500);
		t.send("~click", {
			props: {
				x: Math.round(event.clientX - document.documentElement.clientWidth / 2),
				y: Math.round(event.pageY),
				w: innerWidth,
				s: path(target),
				fx: share(event.clientX, box.left, box.width),
				fy: share(event.clientY, box.top, box.height),
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
