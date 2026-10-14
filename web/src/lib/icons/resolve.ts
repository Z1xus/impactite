import {
	siAndroid,
	siApple,
	siBaidu,
	siBluesky,
	siBrave,
	siClaude,
	siDevdotto,
	siDiscord,
	siDuckduckgo,
	siEcosia,
	siFacebook,
	siFirefoxbrowser,
	siGithub,
	siGmail,
	siGoogle,
	siGooglechrome,
	siGooglegemini,
	siHarmonyos,
	siInstagram,
	siKagi,
	siLinux,
	siLobsters,
	siMastodon,
	siMedium,
	siOpera,
	siPerplexity,
	siPinterest,
	siProducthunt,
	siReddit,
	siSafari,
	siStackoverflow,
	siTelegram,
	siThreads,
	siTiktok,
	siTwitch,
	siVimeo,
	siVivaldi,
	siVk,
	siX,
	siYcombinator,
	siYoutube
} from "simple-icons";
import cursor from "pixelarticons/svg/arrow-right.svg?raw";
import dollar from "pixelarticons/svg/dollar.svg?raw";
import link from "pixelarticons/svg/link.svg?raw";
import mail from "pixelarticons/svg/mail.svg?raw";
import monitor from "pixelarticons/svg/monitor.svg?raw";
import music from "pixelarticons/svg/music.svg?raw";
import message from "pixelarticons/svg/message.svg?raw";
import bell from "pixelarticons/svg/bell.svg?raw";
import robot from "pixelarticons/svg/robot.svg?raw";
import search from "pixelarticons/svg/search.svg?raw";
import cart from "pixelarticons/svg/shopping-cart.svg?raw";
import smartphone from "pixelarticons/svg/smartphone.svg?raw";
import tablet from "pixelarticons/svg/tablet.svg?raw";
import users from "pixelarticons/svg/users.svg?raw";
import video from "pixelarticons/svg/video.svg?raw";
export type Spec =
	| { kind: "brand"; path: string; hex: string }
	| { kind: "image"; url: string; flag?: boolean }
	| { kind: "pixel"; svg: string };

type Brand = { path: string; hex: string };

const brand = (icon: Brand): Spec => ({ kind: "brand", path: icon.path, hex: icon.hex });
const favicon = (host: string): Spec => ({ kind: "image", url: `/api/app/icon/${host}` });
const pixel = (svg: string): Spec => ({ kind: "pixel", svg });

const WINDOWS: Spec = {
	kind: "brand",
	path: "M1 3h10v8H1zM13 3h10v8H13zM1 13h10v8H1zM13 13h10v8H13z",
	hex: "29ADFF"
};

const SOURCES: Record<string, Spec> = {
	Google: brand(siGoogle),
	"Hacker News": brand(siYcombinator),
	Reddit: brand(siReddit),
	Twitter: brand(siX),
	GitHub: brand(siGithub),
	ChatGPT: favicon("chatgpt.com"),
	Perplexity: brand(siPerplexity),
	Claude: brand(siClaude),
	Gemini: brand(siGooglegemini),
	Copilot: favicon("copilot.microsoft.com"),
	DuckDuckGo: brand(siDuckduckgo),
	Bing: favicon("bing.com"),
	"Yahoo!": favicon("yahoo.com"),
	Yandex: favicon("yandex.com"),
	Baidu: brand(siBaidu),
	Ecosia: brand(siEcosia),
	Brave: brand(siBrave),
	Kagi: brand(siKagi),
	LinkedIn: favicon("linkedin.com"),
	Facebook: brand(siFacebook),
	Instagram: brand(siInstagram),
	Youtube: brand(siYoutube),
	Twitch: brand(siTwitch),
	Vimeo: brand(siVimeo),
	TikTok: brand(siTiktok),
	Threads: brand(siThreads),
	Bluesky: brand(siBluesky),
	Mastodon: brand(siMastodon),
	Discord: brand(siDiscord),
	Telegram: brand(siTelegram),
	VKontakte: brand(siVk),
	Pinterest: brand(siPinterest),
	Lobsters: brand(siLobsters),
	"Product Hunt": brand(siProducthunt),
	Medium: brand(siMedium),
	"DEV Community": brand(siDevdotto),
	"Stack Overflow": brand(siStackoverflow),
	Gmail: brand(siGmail),
	"Direct / None": pixel(cursor)
};

const BROWSERS: Record<string, Spec> = {
	Chrome: brand(siGooglechrome),
	Chromium: brand(siGooglechrome),
	Firefox: brand(siFirefoxbrowser),
	Safari: brand(siSafari),
	"Microsoft Edge": favicon("microsoft.com"),
	Opera: brand(siOpera),
	"Yandex Browser": favicon("yandex.com"),
	Vivaldi: brand(siVivaldi),
	Brave: brand(siBrave),
	"DuckDuckGo Privacy Browser": brand(siDuckduckgo),
	Facebook: brand(siFacebook),
	Instagram: brand(siInstagram)
};

const SYSTEMS: Record<string, Spec> = {
	Windows: WINDOWS,
	Mac: brand(siApple),
	iOS: brand(siApple),
	iPadOS: brand(siApple),
	Android: brand(siAndroid),
	"GNU/Linux": brand(siLinux),
	"Chrome OS": brand(siGooglechrome),
	HarmonyOS: brand(siHarmonyos)
};

const DEVICES: Record<string, Spec> = {
	Desktop: pixel(monitor),
	Mobile: pixel(smartphone),
	Tablet: pixel(tablet)
};

const CHANNELS: Record<string, Spec> = {
	"Organic Search": pixel(search),
	"Paid Search": pixel(dollar),
	"Organic Social": pixel(users),
	"Paid Social": pixel(dollar),
	"Organic Video": pixel(video),
	"Paid Video": pixel(dollar),
	"Organic Shopping": pixel(cart),
	"Paid Shopping": pixel(dollar),
	"Paid Other": pixel(dollar),
	"AI Assistants": pixel(robot),
	Email: pixel(mail),
	Referral: pixel(link),
	Direct: pixel(cursor),
	Affiliates: pixel(link),
	Audio: pixel(music),
	SMS: pixel(message),
	"Mobile Push Notifications": pixel(bell)
};

const flags = import.meta.glob("../../../node_modules/flag-icons/flags/4x3/*.svg", {
	query: "?url&no-inline",
	import: "default",
	eager: true
}) as Record<string, string>;

function flag(code: string): Spec | null {
	const url = flags[`../../../node_modules/flag-icons/flags/4x3/${code.toLowerCase()}.svg`];
	return url ? { kind: "image", url, flag: true } : null;
}

const DOMAIN = /^[a-z0-9-]+(\.[a-z0-9-]+)+$/i;

export function icon(dim: string, value: string): Spec | null {
	switch (dim) {
		case "visit:source":
		case "visit:utm_source":
			return SOURCES[value] ?? (DOMAIN.test(value) ? favicon(value) : null);
		case "visit:referrer":
		case "event:hostname": {
			const host = value.split("/")[0];
			return DOMAIN.test(host) ? favicon(host) : null;
		}
		case "visit:channel":
			return CHANNELS[value] ?? null;
		case "visit:browser":
			return BROWSERS[value] ?? null;
		case "visit:os":
			return SYSTEMS[value] ?? null;
		case "visit:device":
			return DEVICES[value] ?? null;
		case "visit:country":
			return flag(value);
		case "visit:region":
			return flag(value.split("-")[0]);
		default:
			return null;
	}
}
