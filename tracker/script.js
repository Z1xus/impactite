(() => {
	const w = window;
	const d = document;
	const l = location;
	const s = d.currentScript;
	const flags = new URL(s.src).pathname.split("/").pop().split(".");
	const has = (flag) => flags.includes(flag);
	const attr = (name) => s.getAttribute(name);
	const o = {
		endpoint: attr("data-api") || `${new URL(s.src).origin}/api/event`,
		domain: attr("data-domain") || l.hostname.replace(/^www\./, ""),
		hashBasedRouting: has("hash"),
		outboundLinks: has("outbound-links"),
		fileDownloads: has("file-downloads"),
		formSubmissions: has("form-submissions"),
		autoCapturePageviews: !has("manual"),
		captureOnLocalhost: has("local"),
		...w.plausible?.o,
	};
	const downloads =
		/\.(pdf|xlsx?|docx?|txt|rtf|csv|exe|key|pps|pptx?|7z|pkg|rar|gz|zip|avi|mov|mp4|mpe?g|wmv|midi?|mp3|wav|wma|dmg)$/i;
	const glob = (pattern) => {
		const escaped = pattern.trim().replace(/[.+^${}()|[\]\\]/g, "\\$&");
		const wild = escaped
			.replace(/\*\*/g, "\0")
			.replace(/\*/g, "[^/]*")
			.replace(/\0/g, ".*");
		return new RegExp("^" + wild + "/?$");
	};
	const matches = (list, path) =>
		list.split(",").some((pattern) => glob(pattern).test(path));

	function ignored() {
		const local =
			/^(localhost|127(\.\d+){3}|\[::1\])$/.test(l.hostname) ||
			l.protocol === "file:";
		if (local && !o.captureOnLocalhost) return true;
		if (w._phantom || w.__nightmare || w.navigator.webdriver || w.Cypress)
			return true;
		try {
			if (w.localStorage.plausible_ignore === "true") return true;
		} catch {}
		const path = l.pathname + (o.hashBasedRouting ? l.hash : "");
		const include = attr("data-include");
		const exclude = attr("data-exclude");
		return (
			(include && !matches(include, path)) ||
			(exclude && matches(exclude, path))
		);
	}

	function send(name, options = {}) {
		const done = options.callback || (() => {});
		if (ignored()) return done();
		const custom = o.customProperties;
		const props = {
			...(typeof custom === "function" ? custom(name) : custom),
			...options.props,
		};
		const payload = {
			n: name,
			u: options.u || l.href,
			d: o.domain,
			r: d.referrer || null,
		};
		if (o.hashBasedRouting) payload.h = 1;
		if (Object.keys(props).length) payload.p = props;
		if (options.revenue) payload.$ = options.revenue;
		if (options.interactive === false) payload.i = false;
		if (name === "engagement")
			Object.assign(payload, { sd: options.sd, e: options.e });
		fetch(o.endpoint, {
			method: "POST",
			headers: { "Content-Type": "text/plain" },
			keepalive: true,
			body: JSON.stringify(payload),
		}).then(
			(response) => done({ status: response.status }),
			(error) => done({ error }),
		);
	}

	let page;
	let view;
	let depth = 0;
	let sent = 0;
	let time = 0;
	let since = 0;

	const active = () => d.visibilityState === "visible" && d.hasFocus();
	const scrolled = () => {
		const height = Math.max(
			d.body?.scrollHeight || 0,
			d.documentElement.scrollHeight,
		);
		return Math.min(
			100,
			Math.round(((w.scrollY + w.innerHeight) / height) * 100),
		);
	};

	function tick() {
		if (since) time += Date.now() - since;
		since = active() ? Date.now() : 0;
	}

	function engage() {
		if (!view) return;
		tick();
		if (depth > sent || time >= 3000) {
			send("engagement", { ...view, sd: depth, e: time });
			sent = depth;
			time = 0;
		}
	}

	function track(name, options = {}) {
		if (name === "pageview") {
			engage();
			const props = { ...options.props };
			for (const { name, value } of s.attributes)
				if (name.startsWith("event-")) props[name.slice(6)] ??= value;
			options = { ...options, props };
			view = { u: options.u || l.href, props };
			depth = scrolled();
			sent = 0;
			time = 0;
			since = active() ? Date.now() : 0;
		}
		send(name, options);
	}

	function auto() {
		const key = l.pathname + (o.hashBasedRouting ? l.hash : "");
		if (key === page) return;
		page = key;
		track("pageview");
	}

	function tagged(element) {
		const tag = element?.closest("[class*='plausible-event-name=']");
		if (!tag) return false;
		let name;
		const props = {};
		for (const token of tag.classList) {
			const [, key, value] =
				token.match(/^plausible-event-(.+?)=(.+)$/) || [];
			if (key === "name") name = value.replace(/\+/g, " ");
			else if (key) props[key] = value.replace(/\+/g, " ");
		}
		if (name) track(name, { props });
		return Boolean(name);
	}

	function click(event) {
		if (event.type === "auxclick" && event.button !== 1) return;
		if (tagged(event.target)) return;
		const link = event.target.closest?.("a[href]");
		if (!link) return;
		if (o.outboundLinks && link.host && link.host !== l.host)
			track("Outbound Link: Click", { props: { url: link.href } });
		else if (o.fileDownloads && downloads.test(link.pathname))
			track("File Download", { props: { url: link.href } });
	}

	function submit(event) {
		if (tagged(event.target)) return;
		if (o.formSubmissions)
			track("Form: Submission", { props: { path: l.pathname } });
	}

	const queue = w.plausible?.q || [];
	w.plausible = track;
	w.plausible.init = (options) => Object.assign(o, options);
	w.plausible.l = true;

	d.addEventListener("click", click);
	d.addEventListener("auxclick", click);
	d.addEventListener("submit", submit);
	w.addEventListener("scroll", () => (depth = Math.max(depth, scrolled())), {
		passive: true,
	});
	w.addEventListener("focus", tick);
	w.addEventListener("blur", tick);
	w.addEventListener("pagehide", engage);
	d.addEventListener("visibilitychange", () =>
		d.visibilityState === "hidden" ? engage() : tick(),
	);

	if (o.autoCapturePageviews) {
		if (o.hashBasedRouting) w.addEventListener("hashchange", auto);
		const push = history.pushState;
		history.pushState = function (...args) {
			push.apply(this, args);
			auto();
		};
		w.addEventListener("popstate", auto);
		if (d.visibilityState === "prerender")
			d.addEventListener(
				"visibilitychange",
				() => d.visibilityState === "visible" && auto(),
			);
		else auto();
	}
	for (const args of queue) track(...args);
})();
