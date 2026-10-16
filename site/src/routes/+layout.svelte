<script lang="ts">
	import "./layout.css";
	import font from "#lib/fonts/DepartureMono-Regular.woff2?url";

	const SITE = "https://impactite.dev";
	const DESCRIPTION =
		"Self-hosted web analytics in one binary. Works with Plausible and GoatCounter scripts, APIs and exports.";

	let { children } = $props();
</script>

<svelte:head>
	<link rel="preload" href={font} as="font" type="font/woff2" crossorigin="anonymous" />
	<title>impactite: self-hosted web analytics in one binary</title>
	<meta name="description" content={DESCRIPTION} />
	<link rel="canonical" href="{SITE}/" />
	<meta property="og:type" content="website" />
	<meta property="og:url" content="{SITE}/" />
	<meta property="og:title" content="impactite: analytics that land" />
	<meta property="og:description" content={DESCRIPTION} />
	<meta property="og:image" content="{SITE}/og.png" />
	<meta property="og:image:width" content="1200" />
	<meta property="og:image:height" content="630" />
	<meta property="og:site_name" content="impactite" />
	<meta name="twitter:card" content="summary_large_image" />
	<!-- eslint-disable-next-line svelte/no-at-html-tags -- static json built from constants above -->
	{@html `<script type="application/ld+json">${JSON.stringify({
		"@context": "https://schema.org",
		"@type": "SoftwareApplication",
		name: "impactite",
		description: DESCRIPTION,
		url: SITE,
		applicationCategory: "BusinessApplication",
		operatingSystem: "Linux",
		license: "https://www.gnu.org/licenses/agpl-3.0.html",
		offers: { "@type": "Offer", price: "0", priceCurrency: "USD" }
	})}</` + `script>`}
</svelte:head>

{@render children()}
