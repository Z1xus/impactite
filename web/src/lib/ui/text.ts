const ACRONYMS = new Set(["utm", "smtp"]);

export const sentence = (text: string) => {
	const [first, ...rest] = text.split(" ");
	return [
		ACRONYMS.has(first) ? first.toUpperCase() : first.charAt(0).toUpperCase() + first.slice(1),
		...rest
	].join(" ");
};
