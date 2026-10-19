import tailwindcss from "@tailwindcss/vite";
import adapter from "@sveltejs/adapter-static";
import { sveltekit } from "@sveltejs/kit/vite";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

// the landing reuses the dashboard's art and components
const shared = fileURLToPath(new URL("../web/src/lib", import.meta.url));

export default defineConfig({
	server: { fs: { allow: [shared] } },
	plugins: [
		tailwindcss(),
		sveltekit({
			compilerOptions: {
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes("node_modules") ? undefined : true
			},
			adapter: adapter({ precompress: true }),
			inlineStyleThreshold: 60_000,
			alias: { "#lib": shared }
		})
	]
});
