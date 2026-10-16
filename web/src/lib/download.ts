import { ApiError } from "./api";

export function save(name: string, content: Blob) {
	const url = URL.createObjectURL(content);
	const link = document.createElement("a");
	link.href = url;
	link.download = name;
	link.click();
	setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export async function archive(site: string, format: string) {
	const response = await fetch("/api/app/export", {
		method: "POST",
		headers: { "content-type": "application/json" },
		body: JSON.stringify({ site_id: site, date_range: "all", format })
	});
	if (!response.ok) {
		const body = await response.json().catch(() => ({}));
		throw new ApiError(response.status, body.error ?? "The export failed.");
	}
	const name =
		response.headers.get("content-disposition")?.match(/filename="(.+)"/)?.[1] ?? "export.zip";
	save(name, await response.blob());
}
