<script lang="ts">
	import "./layout.css";
	import font from "#lib/fonts/DepartureMono-Regular.woff2?url";
	import og from "../lib/og.png?no-inline";
	import { version } from "../../package.json";
	import { DEMO, FEATURES, REPO } from "../lib/demo.ts";

	const SITE = "https://impactite.dev";
	const TITLE = "impactite: a faster Plausible and GoatCounter alternative";
	const DESCRIPTION =
		"Open source, cookieless web analytics in one binary. A faster self-hosted alternative to Plausible, GoatCounter and Google Analytics. Your snippets keep working.";

	const schema = {
		"@context": "https://schema.org",
		"@graph": [
			{
				"@type": "WebSite",
				"@id": `${SITE}/#website`,
				name: "impactite",
				url: `${SITE}/`,
				description: DESCRIPTION
			},
			{
				"@type": "SoftwareApplication",
				"@id": `${SITE}/#software`,
				name: "impactite",
				description: DESCRIPTION,
				url: `${SITE}/`,
				image: `${SITE}${og}`,
				softwareVersion: version,
				applicationCategory: "BusinessApplication",
				applicationSubCategory: "Web analytics",
				keywords:
					"Plausible alternative, GoatCounter alternative, Google Analytics alternative, Umami alternative, self-hosted analytics, privacy-friendly analytics, cookieless analytics, open source web analytics",
				operatingSystem: "Linux, Docker, Podman",
				downloadUrl: `${REPO}/releases/latest`,
				installUrl: `${SITE}/#install`,
				softwareHelp: `${REPO}#readme`,
				sameAs: [REPO, DEMO],
				featureList: FEATURES.map((feature) => feature.label),
				isAccessibleForFree: true,
				license: "https://www.gnu.org/licenses/agpl-3.0.html",
				offers: { "@type": "Offer", price: "0", priceCurrency: "USD" }
			},
			{
				"@type": "SoftwareSourceCode",
				name: "impactite",
				codeRepository: REPO,
				programmingLanguage: ["Rust", "TypeScript", "Svelte"],
				license: "https://www.gnu.org/licenses/agpl-3.0.html",
				targetProduct: { "@id": `${SITE}/#software` }
			}
		]
	};

	let { children } = $props();
</script>

<svelte:head>
	<link rel="preload" href={font} as="font" type="font/woff2" crossorigin="anonymous" />
	<title>{TITLE}</title>
	<meta name="description" content={DESCRIPTION} />
	<link rel="canonical" href="{SITE}/" />
	<link rel="alternate" type="text/markdown" href="{SITE}/llms.txt" title="llms.txt" />
	<meta property="og:type" content="website" />
	<meta property="og:site_name" content="impactite" />
	<meta property="og:locale" content="en_US" />
	<meta property="og:url" content="{SITE}/" />
	<meta property="og:title" content={TITLE} />
	<meta property="og:description" content={DESCRIPTION} />
	<meta property="og:image" content="{SITE}{og}" />
	<meta property="og:image:width" content="1200" />
	<meta property="og:image:height" content="630" />
	<meta
		property="og:image:alt"
		content="Analytics that land. impactite logo with a pixel crater"
	/>
	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:title" content={TITLE} />
	<meta name="twitter:description" content={DESCRIPTION} />
	<meta name="twitter:image" content="{SITE}{og}" />
	<!-- eslint-disable-next-line svelte/no-at-html-tags -- static json built from constants above -->
	{@html `<script type="application/ld+json">${JSON.stringify(schema)}</` + `script>`}
</svelte:head>

{@render children()}
