export function paths(pixels: Uint8Array, width: number, colors: number) {
	const paths = new Array<string>(colors).fill("");
	for (let i = 0; i < pixels.length;) {
		const color = pixels[i];
		const x = i % width;
		const y = Math.floor(i / width);
		let length = 1;
		while (x + length < width && pixels[i + length] === color) length++;
		if (color) paths[color - 1] += `M${x} ${y}h${length}v1h-${length}z`;
		i += length;
	}
	return paths;
}

export function paint(
	ctx: CanvasRenderingContext2D,
	pixels: Uint8Array,
	width: number,
	palette: number[][]
) {
	const image = ctx.createImageData(width, pixels.length / width);
	for (let i = 0; i < pixels.length; i++) {
		if (!pixels[i]) continue;
		image.data.set(palette[pixels[i] - 1], i * 4);
		image.data[i * 4 + 3] = 255;
	}
	ctx.putImageData(image, 0, 0);
}
