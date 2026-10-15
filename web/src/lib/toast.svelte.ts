export const toasts = $state<{ id: number; text: string; bad: boolean }[]>([]);
let next = 0;

export function toast(text: string, bad = false) {
	if (toasts.some((item) => item.text === text)) return;
	const id = next++;
	toasts.push({ id, text, bad });
	setTimeout(() => dismiss(id), bad ? 8000 : 4000);
}

export function dismiss(id: number) {
	const at = toasts.findIndex((item) => item.id === id);
	if (at >= 0) toasts.splice(at, 1);
}

export async function attempt(action: () => Promise<unknown>) {
	try {
		await action();
	} catch (err) {
		toast(err instanceof Error ? err.message : "Something went wrong.", true);
	}
}

// the clipboard api only exists on https and localhost
export async function copy(text: string) {
	try {
		await navigator.clipboard.writeText(text);
		return true;
	} catch {
		const area = document.createElement("textarea");
		area.value = text;
		document.body.append(area);
		area.select();
		const done = document.execCommand("copy");
		area.remove();
		if (!done) toast("Couldn't copy. Select the text and copy it yourself.", true);
		return done;
	}
}
