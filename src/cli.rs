use std::io::IsTerminal;

pub fn say(symbol: char, text: &str) {
	let color = std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
	let paint = |text: &str, code: &str| {
		if color {
			format!("\x1b[{code}m{text}\x1b[0m")
		} else {
			text.to_owned()
		}
	};
	let code = match symbol {
		'+' => "1;92",
		'-' => "1;91",
		'!' => "1;93",
		_ => "1;96",
	};
	let text = text.replace("  |  ", &paint("  |  ", "2"));
	eprintln!("{} {text}", paint(&format!("[{symbol}]"), code));
}
