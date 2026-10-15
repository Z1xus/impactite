// standalone goatcounter count.js replacement, served at /count.js and not joined with the modules
(() => {
	const w = window;
	const d = document;
	const l = location;
	const s = d.querySelector("script[data-goatcounter]");
	const self = d.currentScript;
	const gc = (w.goatcounter ||= {});
	try {
		Object.assign(gc, JSON.parse(s?.dataset.goatcounterSettings || "{}"));
	} catch {}
	const endpoint = () =>
		gc.endpoint ||
		s?.dataset.goatcounter ||
		new URL("/count", self?.src || l.href).href;
	const value = (setting, fallback) =>
		typeof setting === "function" ? setting(fallback) : setting ?? fallback;

	const bot = () =>
		w.callPhantom || w._phantom || w.phantom
			? 150
			: w.__nightmare
				? 151
				: d.__selenium_unwrapped || d.__webdriver_evaluate || d.__driver_evaluate
					? 152
					: navigator.webdriver
						? 153
						: 0;

	gc.filter = () => {
		if (d.visibilityState === "prerender") return "visibilityState";
		if (!gc.allow_frame && w.top !== w) return "frame";
		if (
			!gc.allow_local &&
			/^(localhost|127\.|10\.|172\.(1[6-9]|2\d|3[01])\.|192\.168\.|0\.0\.0\.0|\[::1\])/.test(l.hostname)
		)
			return "localhost";
		if (!gc.allow_local && l.protocol === "file:") return "localhost";
		try {
			if (localStorage.getItem("skipgc") === "t") return "disabled with #toggle-goatcounter";
		} catch {}
		return false;
	};

	gc.get_query = (name) => new URLSearchParams(l.search).get(name);

	gc.url = (vars = {}) => {
		const canonical = d.querySelector('link[rel="canonical"][href]');
		const here = canonical ? new URL(canonical.href, l.href) : l;
		const event = vars.event ?? gc.event;
		const data = {
			p: value(vars.path ?? gc.path, event ? "" : here.pathname + here.search),
			r: value(vars.referrer ?? gc.referrer, d.referrer),
			t: value(vars.title ?? gc.title, d.title),
			e: !!event,
			s: [screen.width, screen.height, w.devicePixelRatio || 1].join(","),
			b: bot(),
			q: l.search,
			rnd: Math.random().toString(36).slice(2),
		};
		if (data.p == null) return;
		const url = new URL(endpoint());
		for (const [key, item] of Object.entries(data)) url.searchParams.set(key, item);
		return url.href;
	};

	gc.count = (vars = {}) => {
		const reason = gc.filter();
		if (reason) return console.log?.("goatcounter: not counting because of: " + reason);
		const url = gc.url(vars);
		if (!url) return;
		if (!navigator.sendBeacon?.(url)) new Image().src = url;
	};

	gc.bind_events = () => {
		for (const element of d.querySelectorAll("[data-goatcounter-click]")) {
			if (element.dataset.goatcounterBound) continue;
			element.dataset.goatcounterBound = "true";
			const send = (event) => {
				if (event.type === "auxclick" && event.button !== 1) return;
				const data = element.dataset;
				gc.count({
					event: true,
					path: data.goatcounterClick || element.name || element.id || "",
					title: data.goatcounterTitle || element.title || (element.innerHTML || "").slice(0, 200),
					referrer: data.goatcounterReferrer || data.goatcounterReferral || "",
				});
			};
			element.addEventListener("click", send, false);
			element.addEventListener("auxclick", send, false);
		}
	};

	gc.visit_count = (opt = {}) => {
		const type = opt.type || "html";
		const url = new URL(
			`${opt.base || endpoint().replace(/\/count$/, "")}/counter/${encodeURIComponent(opt.path || l.pathname)}.${type}`,
		);
		url.searchParams.set("site", l.hostname);
		for (const key of ["start", "end", "style"]) if (opt[key]) url.searchParams.set(key, opt[key]);
		if (opt.no_branding) url.searchParams.set("no_branding", 1);
		const element = d.createElement(type === "html" ? "iframe" : "img");
		element.src = url.href;
		if (type === "html") {
			element.style.border = "0";
			element.width = 200;
			element.height = opt.no_branding ? 60 : 80;
		}
		for (const [name, item] of Object.entries(opt.attr || {})) element.setAttribute(name, item);
		d.querySelector(opt.append || "body")?.append(element);
	};

	const start = () => {
		if (!gc.no_onload) {
			if (d.visibilityState === "prerender") {
				const once = () => {
					if (d.visibilityState === "prerender") return;
					d.removeEventListener("visibilitychange", once);
					gc.count();
				};
				d.addEventListener("visibilitychange", once);
			} else gc.count();
		}
		if (!gc.no_events) gc.bind_events();
	};
	if (d.body === null) d.addEventListener("DOMContentLoaded", start, false);
	else start();
})();
