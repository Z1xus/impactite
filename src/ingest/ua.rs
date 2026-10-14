#[derive(Debug, Default, PartialEq, Eq)]
pub struct Agent {
	pub browser: &'static str,
	pub browser_version: String,
	pub os: &'static str,
	pub os_version: String,
	pub device: &'static str,
}

const BOTS: &[&str] = &[
	"bot",
	"crawl",
	"spider",
	"slurp",
	"headless",
	"lighthouse",
	"preview",
	"python",
	"curl/",
	"wget/",
	"go-http",
	"java/",
	"okhttp",
	"axios/",
	"node-fetch",
	"undici",
	"scrapy",
	"phantomjs",
	"selenium",
	"puppeteer",
	"playwright",
	"httpclient",
	"monitor",
	"uptime",
	"pingdom",
	"facebookexternalhit",
	"whatsapp/",
	"embedly",
	"ahrefs",
	"semrush",
	"petal",
	"bytespider",
	"gptbot",
	"claude-web",
	"perplexity",
	"chatgpt-user",
	"cfnetwork",
	"dalvik",
	"feed",
	"rss",
];

const BROWSERS: &[(&str, &str)] = &[
	("Edg/", "Microsoft Edge"),
	("EdgA/", "Microsoft Edge"),
	("EdgiOS/", "Microsoft Edge"),
	("OPR/", "Opera"),
	("OPiOS/", "Opera"),
	("SamsungBrowser/", "Samsung Internet"),
	("YaBrowser/", "Yandex Browser"),
	("UCBrowser/", "UC Browser"),
	("Vivaldi/", "Vivaldi"),
	("DuckDuckGo/", "DuckDuckGo Privacy Browser"),
	("Ddg/", "DuckDuckGo Privacy Browser"),
	("FBAV/", "Facebook"),
	("Instagram ", "Instagram"),
	("FxiOS/", "Firefox"),
	("Firefox/", "Firefox"),
	("CriOS/", "Chrome"),
	("Chromium/", "Chromium"),
	("Chrome/", "Chrome"),
	("Version/", "Safari"),
];

pub fn is_bot(ua: &str) -> bool {
	let ua = ua.to_ascii_lowercase();
	ua.len() < 16 || BOTS.iter().any(|bot| ua.contains(bot))
}

pub fn parse(ua: &str) -> Agent {
	let mut agent = Agent::default();

	if let Some((token, name)) = BROWSERS.iter().find(|(token, _)| ua.contains(token)) {
		agent.browser = name;
		agent.browser_version = version_after(ua, token);
	}

	if let Some(rest) = after(ua, "Windows NT ") {
		agent.os = "Windows";
		agent.os_version = match number(rest).as_str() {
			"10.0" => "10".into(),
			"6.3" => "8.1".into(),
			"6.2" => "8".into(),
			"6.1" => "7".into(),
			other => other.into(),
		};
	} else if ua.contains("iPad") {
		agent.os = "iPadOS";
		agent.os_version = version_after(ua, "CPU OS ");
	} else if ua.contains("iPhone") || ua.contains("iPod") {
		agent.os = "iOS";
		agent.os_version = version_after(ua, "iPhone OS ");
	} else if ua.contains("Android") {
		agent.os = "Android";
		agent.os_version = version_after(ua, "Android ");
	} else if ua.contains("CrOS") {
		agent.os = "Chrome OS";
	} else if ua.contains("Mac OS X") {
		agent.os = "Mac";
		agent.os_version = version_after(ua, "Mac OS X ");
	} else if ua.contains("HarmonyOS") {
		agent.os = "HarmonyOS";
	} else if ua.contains("Linux") {
		agent.os = "GNU/Linux";
	}

	agent.device = if ua.contains("iPad")
		|| ua.contains("Tablet")
		|| (ua.contains("Android") && !ua.contains("Mobile"))
	{
		"Tablet"
	} else if ua.contains("Mobi") || ua.contains("iPhone") || ua.contains("iPod") {
		"Mobile"
	} else {
		"Desktop"
	};

	if agent.browser == "Safari" && !ua.contains("Safari/") {
		agent.browser = "";
		agent.browser_version.clear();
	}
	agent
}

fn after<'a>(ua: &'a str, token: &str) -> Option<&'a str> {
	ua.find(token).map(|i| &ua[i + token.len()..])
}

fn version_after(ua: &str, token: &str) -> String {
	after(ua, token).map_or_else(String::new, number)
}

fn number(rest: &str) -> String {
	let mut parts = rest
		.split(|c: char| !c.is_ascii_digit() && c != '.' && c != '_')
		.next()
		.unwrap_or_default()
		.split(['.', '_'])
		.filter(|part| !part.is_empty());
	match (parts.next(), parts.next()) {
		(Some(major), Some(minor)) => format!("{major}.{minor}"),
		(Some(major), None) => major.to_owned(),
		_ => String::new(),
	}
}
