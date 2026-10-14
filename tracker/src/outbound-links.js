m.push((t) =>
	document.addEventListener("click", (event) => {
		const link = event.target.closest?.("a[href]");
		if (link?.host && link.host != location.host)
			t.track("Outbound Link: Click", { props: { url: link.href } });
	}),
);
