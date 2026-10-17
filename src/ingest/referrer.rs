#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
	Search,
	Social,
	Video,
	Ai,
	Email,
	Shopping,
	Other,
}

const SOURCES: &[(&str, &str, Kind)] = &[
	("google.", "Google", Kind::Search),
	("bing.com", "Bing", Kind::Search),
	("duckduckgo.com", "DuckDuckGo", Kind::Search),
	("search.yahoo.", "Yahoo!", Kind::Search),
	("yandex.", "Yandex", Kind::Search),
	("ya.ru", "Yandex", Kind::Search),
	("baidu.com", "Baidu", Kind::Search),
	("ecosia.org", "Ecosia", Kind::Search),
	("search.brave.com", "Brave", Kind::Search),
	("startpage.com", "Startpage", Kind::Search),
	("qwant.com", "Qwant", Kind::Search),
	("kagi.com", "Kagi", Kind::Search),
	("naver.com", "Naver", Kind::Search),
	("seznam.cz", "Seznam", Kind::Search),
	("t.co", "Twitter", Kind::Social),
	("twitter.com", "Twitter", Kind::Social),
	("x.com", "Twitter", Kind::Social),
	("facebook.com", "Facebook", Kind::Social),
	("fb.com", "Facebook", Kind::Social),
	("instagram.com", "Instagram", Kind::Social),
	("linkedin.com", "LinkedIn", Kind::Social),
	("lnkd.in", "LinkedIn", Kind::Social),
	("reddit.com", "Reddit", Kind::Social),
	("news.ycombinator.com", "Hacker News", Kind::Social),
	("pinterest.", "Pinterest", Kind::Social),
	("tiktok.com", "TikTok", Kind::Social),
	("threads.net", "Threads", Kind::Social),
	("threads.com", "Threads", Kind::Social),
	("bsky.app", "Bluesky", Kind::Social),
	("mastodon.social", "Mastodon", Kind::Social),
	("discord.com", "Discord", Kind::Social),
	("t.me", "Telegram", Kind::Social),
	("vk.com", "VKontakte", Kind::Social),
	("lobste.rs", "Lobsters", Kind::Social),
	("producthunt.com", "Product Hunt", Kind::Social),
	("youtube.com", "Youtube", Kind::Video),
	("youtu.be", "Youtube", Kind::Video),
	("vimeo.com", "Vimeo", Kind::Video),
	("twitch.tv", "Twitch", Kind::Video),
	("chatgpt.com", "ChatGPT", Kind::Ai),
	("chat.openai.com", "ChatGPT", Kind::Ai),
	("perplexity.ai", "Perplexity", Kind::Ai),
	("claude.ai", "Claude", Kind::Ai),
	("gemini.google.com", "Gemini", Kind::Ai),
	("copilot.microsoft.com", "Copilot", Kind::Ai),
	("mail.google.com", "Gmail", Kind::Email),
	("outlook.live.com", "Outlook.com", Kind::Email),
	("mail.yahoo.com", "Yahoo! Mail", Kind::Email),
	("shopping.google.", "Google Shopping", Kind::Shopping),
	("amazon.", "Amazon", Kind::Shopping),
	("github.com", "GitHub", Kind::Other),
	("stackoverflow.com", "Stack Overflow", Kind::Other),
	("medium.com", "Medium", Kind::Other),
	("dev.to", "DEV Community", Kind::Other),
];

pub struct Traffic<'a> {
	pub referrer_host: &'a str,
	pub utm_source: &'a str,
	pub utm_medium: &'a str,
	pub click_id: bool,
}

pub fn source(referrer_host: &str, tagged: &str) -> String {
	if !tagged.is_empty() {
		let lower = tagged.to_ascii_lowercase();
		return SOURCES
			.iter()
			.find(|(host, name, _)| {
				name.eq_ignore_ascii_case(&lower)
					|| host.trim_end_matches('.') == lower
					|| alias(&lower) == Some(*name)
			})
			.map_or_else(|| tagged.to_owned(), |(_, name, _)| (*name).to_owned());
	}
	lookup(referrer_host).map_or_else(|| referrer_host.to_owned(), |(name, _)| name.to_owned())
}

pub fn channel(traffic: &Traffic) -> &'static str {
	let medium = traffic.utm_medium.to_ascii_lowercase();
	let kind = lookup(traffic.referrer_host)
		.map(|(_, kind)| kind)
		.or_else(|| {
			let tagged = source(traffic.referrer_host, traffic.utm_source);
			SOURCES
				.iter()
				.find(|(_, name, _)| *name == tagged)
				.map(|&(_, _, kind)| kind)
		})
		.unwrap_or(Kind::Other);
	let paid = traffic.click_id
		|| medium.contains("cp")
		|| medium == "ppc"
		|| medium == "retargeting"
		|| medium.starts_with("paid");

	match (paid, kind) {
		(true, Kind::Search) => "Paid Search",
		(true, Kind::Social) => "Paid Social",
		(true, Kind::Video) => "Paid Video",
		(true, Kind::Shopping) => "Paid Shopping",
		(true, _) => "Paid Other",
		(false, Kind::Ai) => "AI Assistants",
		(false, Kind::Search) => "Organic Search",
		(false, Kind::Shopping) => "Organic Shopping",
		(false, Kind::Social) => "Organic Social",
		(false, Kind::Video) => "Organic Video",
		(false, Kind::Email) => "Email",
		_ if matches!(
			medium.as_str(),
			"social" | "social-network" | "social-media" | "sm"
		) =>
		{
			"Organic Social"
		}
		_ if medium.contains("video") => "Organic Video",
		_ if matches!(medium.as_str(), "email" | "e-mail" | "e_mail" | "e mail") => "Email",
		_ if medium == "affiliate" => "Affiliates",
		_ if medium == "audio" => "Audio",
		_ if medium == "sms" => "SMS",
		_ if medium.ends_with("push")
			|| medium.contains("mobile")
			|| medium.contains("notification") =>
		{
			"Mobile Push Notifications"
		}
		_ if !traffic.referrer_host.is_empty()
			|| !traffic.utm_source.is_empty()
			|| matches!(medium.as_str(), "referral" | "app" | "link") =>
		{
			"Referral"
		}
		_ => "Direct",
	}
}

fn lookup(host: &str) -> Option<(&'static str, Kind)> {
	if host.is_empty() {
		return None;
	}
	SOURCES
		.iter()
		.filter(|(pattern, _, _)| matches(host, pattern))
		.max_by_key(|(pattern, _, _)| pattern.len())
		.map(|&(_, name, kind)| (name, kind))
}

fn matches(host: &str, pattern: &str) -> bool {
	let labels = |host: &str| {
		host.match_indices('.')
			.map(|(i, _)| i + 1)
			.collect::<Vec<_>>()
	};
	let starts = std::iter::once(0).chain(labels(host));
	starts.into_iter().any(|start| {
		let rest = &host[start..];
		if pattern.ends_with('.') {
			rest.starts_with(pattern)
		} else {
			rest == pattern
		}
	})
}

fn alias(tagged: &str) -> Option<&'static str> {
	Some(match tagged {
		"ig" => "Instagram",
		"fb" => "Facebook",
		"tw" | "x" => "Twitter",
		"yt" => "Youtube",
		"li" => "LinkedIn",
		"hn" => "Hacker News",
		"adwords" | "googleads" => "Google",
		_ => return None,
	})
}
