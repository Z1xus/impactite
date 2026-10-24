<script lang="ts">
	import { untrack } from "svelte";
	import Code from "#lib/ui/Code.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";

	let { change }: { change?: () => void } = $props();

	// everything stays in the browser, secrets are placeholders to fill in later
	let host = $state("");
	let run = $state<"binary" | "docker" | "podman" | "compose">("binary");
	let https = $state<"built-in" | "proxy" | "tunnel">("built-in");
	let mail = $state<"none" | "smtp" | "resend" | "cloudflare">("none");

	const name = $derived(
		host
			.trim()
			.replace(/^https?:\/\//, "")
			.replace(/\/.*$/, "") || "stats.example.com"
	);
	const url = $derived(`https://${name}`);
	const image = "ghcr.io/z1xus/impactite";
	const mailUrl = $derived(
		{
			none: "",
			smtp: "smtp://USER:PASSWORD@SMTP_HOST:587",
			resend: "resend://RESEND_API_KEY",
			cloudflare: "cloudflare://API_TOKEN@ACCOUNT_ID"
		}[mail]
	);
	const acme = $derived(https === "built-in" ? " --acme" : "");

	const command = $derived.by(() => {
		if (run === "binary") {
			const proxy = https === "built-in" ? "" : " --proxy";
			return [
				`curl -fsSL https://impactite.dev/install | sh -s -- --url ${url}${proxy}`,
				...(mailUrl
					? [
							`echo IMPACTITE_MAIL=${mailUrl} | sudo tee -a /etc/impactite/env`,
							"sudo systemctl restart impactite"
						]
					: []),
				...(https === "tunnel"
					? [
							`# route the tunnel to http://localhost:8000 in cloudflare`,
							`cloudflared tunnel run --token TUNNEL_TOKEN`
						]
					: [])
			].join("\n");
		}
		if (run === "docker") {
			const env = mailUrl ? ` -e IMPACTITE_MAIL=${mailUrl}` : "";
			const ports = {
				"built-in": " -p 443:8000 -p 80:80",
				proxy: " -p 127.0.0.1:8000:8000",
				tunnel: " --network impactite"
			}[https];
			return [
				...(https === "tunnel" ? ["docker network create impactite"] : []),
				`docker run -d --name impactite --restart unless-stopped${ports}${env} -v impactite:/data ${image} serve --url ${url}${acme}`,
				...(https === "tunnel"
					? [
							`# route the tunnel to http://impactite:8000 in cloudflare`,
							`docker run -d --restart unless-stopped --network impactite cloudflare/cloudflared tunnel run --token TUNNEL_TOKEN`
						]
					: [])
			].join("\n");
		}
		if (run === "podman") {
			const unit = (name: string, lines: string[]) => [
				`cat > ~/.config/containers/systemd/${name}.container <<'EOF'`,
				"[Container]",
				...lines,
				"ReadOnly=true",
				"DropCapability=ALL",
				"NoNewPrivileges=true",
				"",
				"[Service]",
				"Restart=always",
				"",
				"[Install]",
				"WantedBy=default.target",
				"EOF"
			];
			const ports = {
				"built-in": ["PublishPort=443:8000", "PublishPort=80:80"],
				proxy: ["PublishPort=127.0.0.1:8000:8000"],
				tunnel: []
			}[https];
			return [
				...(https === "built-in"
					? [
							"echo net.ipv4.ip_unprivileged_port_start=80 | sudo tee /etc/sysctl.d/impactite.conf",
							"sudo sysctl --system"
						]
					: []),
				"sudo loginctl enable-linger $USER",
				"mkdir -p ~/.config/containers/systemd",
				...unit("impactite", [
					`Image=${image}`,
					`Exec=serve --url ${url}${acme}`,
					"Volume=impactite:/data:U",
					...ports,
					...(mailUrl ? [`Environment=IMPACTITE_MAIL=${mailUrl}`] : []),
					"User=1000:1000",
					"Sysctl=net.ipv4.ip_unprivileged_port_start=0"
				]),
				...(https === "tunnel"
					? [
							"# route the tunnel to http://localhost:8000 in cloudflare",
							...unit("impactite-tunnel", [
								"Image=docker.io/cloudflare/cloudflared",
								"Exec=tunnel --no-autoupdate run --token TUNNEL_TOKEN",
								"Network=impactite.container"
							])
						]
					: []),
				`systemctl --user daemon-reload && systemctl --user start impactite${https === "tunnel" ? " impactite-tunnel" : ""}`
			].join("\n");
		}
		const ports = {
			"built-in": ['      - "443:8000"', '      - "80:80"'],
			proxy: ['      - "127.0.0.1:8000:8000"'],
			tunnel: []
		}[https];
		return [
			"services:",
			"  impactite:",
			`    image: ${image}`,
			`    command: serve --url ${url}${acme}`,
			"    restart: unless-stopped",
			...(mailUrl ? ["    environment:", `      IMPACTITE_MAIL: ${mailUrl}`] : []),
			...(ports.length ? ["    ports:", ...ports] : []),
			"    volumes:",
			"      - impactite:/data",
			...(https === "tunnel"
				? [
						"  # route the tunnel to http://impactite:8000 in cloudflare",
						"  cloudflared:",
						"    image: cloudflare/cloudflared",
						"    command: tunnel run --token TUNNEL_TOKEN",
						"    restart: unless-stopped"
					]
				: []),
			"volumes:",
			"  impactite:"
		].join("\n");
	});

	const end = "</" + "script>";
	const snippet = $derived(
		`<script defer data-domain="example.com" src="${url}/js/script.js">${end}`
	);

	$effect(() => {
		void command;
		untrack(() => change?.());
	});
</script>

<div class="flex flex-col gap-6">
	<Field label="your stats address" placeholder="stats.example.com" bind:value={host} />
	<div class="flex flex-wrap gap-x-6 gap-y-4">
		<Toggle
			label="run as"
			bind:value={run}
			options={[
				{ key: "binary", label: "binary" },
				{ key: "docker", label: "docker" },
				{ key: "podman", label: "podman" },
				{ key: "compose", label: "compose" }
			]}
		/>
		<Toggle
			label="https"
			bind:value={https}
			options={[
				{ key: "built-in", label: "built in" },
				{ key: "proxy", label: "my proxy" },
				{ key: "tunnel", label: "cloudflare tunnel" }
			]}
		/>
		<Toggle
			label="mail"
			bind:value={mail}
			options={[
				{ key: "none", label: "none" },
				{ key: "smtp", label: "smtp" },
				{ key: "resend", label: "resend" },
				{ key: "cloudflare", label: "cloudflare" }
			]}
		/>
	</div>
	<Code code={command} />
	<span class="label">then on your site</span>
	<Code code={snippet} />
</div>
