m.push((t) =>
	document.addEventListener("click", (event) => {
		const link = event.target.closest?.("a[href]");
		if (
			/\.(pdf|xlsx?|docx?|txt|rtf|csv|exe|key|pps|pptx?|7z|pkg|rar|gz|zip|avi|mov|mp4|mpe?g|wmv|midi?|mp3|wav|wma|dmg)$/i.test(
				link?.pathname,
			)
		)
			t.track("File Download", { props: { url: link.href } });
	}),
);
