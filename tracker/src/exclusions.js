m.push((t) => {
	const script = document.currentScript;
	const matches = (list, path) =>
		list?.split(",").some((pattern) => {
			const escaped = pattern.trim().replace(/[.+^${}()|[\]\\]/g, "\\$&");
			const wild = escaped
				.replace(/\*\*/g, "\0")
				.replace(/\*/g, "[^/]*")
				.replace(/\0/g, ".*");
			return new RegExp("^" + wild + "/?$").test(path);
		});
	const include = script.getAttribute("data-include");
	const exclude = script.getAttribute("data-exclude");
	t.skip.push((href) => {
		const url = new URL(href);
		const path = url.pathname + (t.extra.h ? url.hash : "");
		return (include && !matches(include, path)) || matches(exclude, path);
	});
});
