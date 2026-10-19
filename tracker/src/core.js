// modules push setup functions into `m`, the server wraps everything in one scope
const w = window;
const d = document;
const l = location;
const s = d.currentScript;
const on = (target, type, listener) => target.addEventListener(type, listener);
const t = {
	endpoint: s.getAttribute("data-api") || new URL("/api/event", s.src),
	domain: s.getAttribute("data-domain") || l.hostname.replace(/^www\./, ""),
	key: () => l.pathname,
	extra: {},
	skip: [],
	auto: 1,
	...w.plausible?.o,
};

const ignored = (url) => {
	try {
		return (
			(/^(localhost|127\.|\[::1\])/.test(l.hostname) && !t.local) ||
			navigator.webdriver ||
			localStorage.impactite_ignore == "true" ||
			localStorage.plausible_ignore == "true" ||
			t.skip.some((test) => test(url))
		);
	} catch {}
};

t.send = (name, o = {}) => {
	const done = o.callback || (() => {});
	const url = o.u || l.href;
	if (ignored(url)) return done();
	fetch(t.endpoint, {
		method: "POST",
		keepalive: 1,
		body: JSON.stringify({
			n: name,
			u: url,
			d: t.domain,
			r: d.referrer,
			p: o.props,
			$: o.revenue,
			i: o.interactive,
			sd: o.sd,
			e: o.e,
			...t.extra,
		}),
	}).then(
		(response) => done({ status: response.status }),
		(error) => done({ error }),
	);
};

let page;
let view;
let depth;
let sent;
let time;
let since;

// the server clamps depth to 100
const scrolled = () =>
	Math.round(
		((scrollY + innerHeight) / d.documentElement.scrollHeight) * 100,
	);

const tick = () => {
	if (since) time += Date.now() - since;
	since = !d.hidden && d.hasFocus() && Date.now();
};

const engage = () => {
	tick();
	if (view && (depth > sent || time > 3e3)) {
		t.send("engagement", { ...view, sd: depth, e: time });
		sent = depth;
		time = 0;
	}
};

t.track = (name, o = {}) => {
	if (name == "pageview") {
		engage();
		view = { u: o.u || l.href, props: o.props };
		depth = scrolled();
		sent = time = since = 0;
		tick();
	}
	t.send(name, o);
};

t.page = () => {
	if (t.key() != page) {
		page = t.key();
		t.track("pageview");
	}
};

for (const setup of m) setup(t);

on(w, "scroll", () => (depth = Math.max(depth, scrolled())));
on(w, "focus", tick);
on(w, "blur", tick);
on(w, "pagehide", engage);
on(d, "visibilitychange", () => (d.hidden ? engage() : tick()));

if (t.auto) {
	const push = history.pushState;
	history.pushState = function (...args) {
		push.apply(this, args);
		t.page();
	};
	on(w, "popstate", t.page);
	d.prerendering ? on(d, "prerenderingchange", t.page) : t.page();
}
for (const args of w.plausible?.q || []) t.track(...args);
w.plausible = (...args) => t.track(...args);
