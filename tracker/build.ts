import { mkdir, readdir } from "node:fs/promises";

await mkdir(`${import.meta.dir}/dist`, { recursive: true });
for (const file of await readdir(`${import.meta.dir}/src`)) {
	const result = await Bun.build({
		entrypoints: [`${import.meta.dir}/src/${file}`],
		minify: true,
	});
	await Bun.write(
		`${import.meta.dir}/dist/${file}`,
		(await result.outputs[0].text()).trim(),
	);
}
