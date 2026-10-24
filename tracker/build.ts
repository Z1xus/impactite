import { mkdir, readdir } from "node:fs/promises";

const dir = import.meta.dir;
await mkdir(`${dir}/dist`, { recursive: true });
// src modules are joined into one script by the server, extras load on their own
for (const [folder, format] of [
	["src", "esm"],
	["extras", "iife"],
] as const) {
	for (const file of await readdir(`${dir}/${folder}`)) {
		const result = await Bun.build({
			entrypoints: [`${dir}/${folder}/${file}`],
			minify: true,
			format,
		});
		await Bun.write(`${dir}/dist/${file}`, (await result.outputs[0].text()).trim());
	}
}
