export function save(name: string, content: Blob) {
	const url = URL.createObjectURL(content);
	const link = document.createElement("a");
	link.href = url;
	link.download = name;
	link.click();
	setTimeout(() => URL.revokeObjectURL(url), 1000);
}
