const proxy = (domain: string, url: string) =>
	/^https?:\/\//i.test(url) ? `/api/app/sites/${domain}/asset?u=${encodeURIComponent(url)}` : url;

const styles = (domain: string, text: string) =>
	text.replace(
		/url\(\s*(['"]?)(https?:\/\/[^'")\s]+)\1\s*\)/gi,
		(_, quote: string, url: string) => `url(${quote}${proxy(domain, url)}${quote})`
	);

const sources = (domain: string, text: string) =>
	text
		.split(",")
		.map((part) => part.trim().replace(/^\S+/, (url) => proxy(domain, url)))
		.join(", ");

const LINKED = new Set(["link", "image", "use"]);

// recorded pages point at their own servers, which the dashboard blocks, so they load through ours
export function localize(domain: string, value: unknown, tag = ""): void {
	if (Array.isArray(value)) {
		for (const item of value) localize(domain, item, tag);
		return;
	}
	if (!value || typeof value !== "object") return;
	const node = value as Record<string, unknown>;
	const name = typeof node.tagName === "string" ? node.tagName : tag;
	for (const [key, item] of Object.entries(node)) {
		if (typeof item !== "string") {
			localize(domain, item, name);
			continue;
		}
		if (key === "src" || key === "poster" || key === "xlink:href")
			node[key] = proxy(domain, item);
		else if (key === "href" && LINKED.has(name)) node[key] = proxy(domain, item);
		else if (key === "srcset") node[key] = sources(domain, item);
		else if ((key !== "textContent" || node.isStyle) && item.includes("url("))
			node[key] = styles(domain, item);
	}
}
