m.push((t) => {
	t.key = () => location.pathname + location.hash;
	t.extra.h = 1;
	addEventListener("hashchange", () => t.auto && t.page());
});
