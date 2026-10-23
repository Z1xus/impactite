import { siDiscord, siNtfy, siTelegram } from "simple-icons";
import link from "pixelarticons/svg/link.svg?raw";
import mail from "pixelarticons/svg/mail.svg?raw";
import type { Spec } from "./icons/resolve.ts";

export type Channel = "email" | "slack" | "discord" | "telegram" | "ntfy" | "webhook";

const brand = (icon: { path: string; hex: string }): Spec => ({ kind: "brand", ...icon });

export const CHANNELS: { kind: Channel; name: string; spec: Spec; example: string }[] = [
	{
		kind: "email",
		name: "Email",
		spec: { kind: "pixel", svg: mail },
		example: "name@example.com"
	},
	{
		kind: "slack",
		name: "Slack",
		spec: { kind: "image", url: "/api/app/icon/slack.com" },
		example: "https://hooks.slack.com/services/…"
	},
	{
		kind: "discord",
		name: "Discord",
		spec: brand(siDiscord),
		example: "https://discord.com/api/webhooks/…"
	},
	{
		kind: "telegram",
		name: "Telegram",
		spec: brand(siTelegram),
		example: "https://api.telegram.org/bot…/sendMessage?chat_id=…"
	},
	{ kind: "ntfy", name: "ntfy", spec: brand(siNtfy), example: "https://ntfy.sh/your-topic" },
	{
		kind: "webhook",
		name: "Webhook",
		spec: { kind: "pixel", svg: link },
		example: "https://example.com/hook"
	}
];

export const channel = (kind: string) => CHANNELS.find((item) => item.kind === kind);

// mirrors notify::classify on the server, which stays the judge
export function detect(raw: string): { kind: Channel | null; error: string } {
	const to = raw.trim();
	if (!to) return { kind: null, error: "" };
	if (!to.includes("://")) {
		return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(to)
			? { kind: "email", error: "" }
			: { kind: null, error: "Enter an email address or an https URL" };
	}
	let url: URL;
	try {
		url = new URL(to);
	} catch {
		return { kind: null, error: "This is not a valid URL" };
	}
	if (url.protocol !== "https:") return { kind: null, error: "Use an https URL" };
	const host = url.hostname;
	const path = url.pathname.replace(/\/+$/, "");
	const parts = path.split("/").length;
	if (host === "hooks.slack.com")
		return path.startsWith("/services/") && parts >= 5
			? { kind: "slack", error: "" }
			: { kind: "slack", error: "Use the whole Slack webhook URL" };
	if (host === "discord.com" || host === "discordapp.com")
		return path.startsWith("/api/webhooks/") && parts >= 5
			? { kind: "discord", error: "" }
			: { kind: "discord", error: "Use the whole Discord webhook URL" };
	if (host === "api.telegram.org") {
		if (!path.startsWith("/bot") || !path.endsWith("/sendMessage"))
			return { kind: "telegram", error: "Use the bot sendMessage URL" };
		return url.searchParams.get("chat_id")
			? { kind: "telegram", error: "" }
			: { kind: "telegram", error: "Add chat_id to the Telegram URL" };
	}
	if (host.startsWith("ntfy.")) {
		const topic = path.replace(/^\//, "");
		return topic && !topic.includes("/")
			? { kind: "ntfy", error: "" }
			: { kind: "ntfy", error: "Add a topic to the ntfy URL" };
	}
	return { kind: "webhook", error: "" };
}
