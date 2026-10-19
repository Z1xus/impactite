import tailwindcss from "@tailwindcss/vite";
import adapter from "@sveltejs/adapter-static";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";

export default defineConfig({
	server: {
		proxy: { "/api": "http://127.0.0.1:8000" }
	},
	plugins: [
		tailwindcss(),
		sveltekit({
			compilerOptions: {
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes("node_modules") ? undefined : true
			},
			adapter: adapter({ fallback: "index.html", precompress: true }),
			output: { bundleStrategy: "single" },
			inlineStyleThreshold: 30_000
		})
	]
});
