// Reads what a canvas drew, for the plot's browser tests.

export type Rgb = readonly [number, number, number];

export interface Pixel {
	rgb: Rgb;
	alpha: number;
}

export const tokenRgb = (name: string): Rgb => {
	const value = getComputedStyle(document.documentElement)
		.getPropertyValue(name)
		.trim();
	const hex = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(value);
	if (!hex) {
		throw new Error(`${name} is not a six digit hex color: ${value}`);
	}
	return [
		Number.parseInt(hex[1] as string, 16),
		Number.parseInt(hex[2] as string, 16),
		Number.parseInt(hex[3] as string, 16),
	];
};

/** The pixel under a point given in client coordinates. */
export const pixelAt = (
	canvas: HTMLCanvasElement,
	clientX: number,
	clientY: number,
): Pixel => {
	const rect = canvas.getBoundingClientRect();
	const ratio = canvas.width / rect.width;
	const x = Math.floor((clientX - rect.left) * ratio);
	const y = Math.floor((clientY - rect.top) * ratio);
	const context = canvas.getContext("2d");
	if (!context) {
		throw new Error("No 2d context");
	}
	const [r = 0, g = 0, b = 0, alpha = 0] = context.getImageData(
		x,
		y,
		1,
		1,
	).data;
	return { rgb: [r, g, b], alpha };
};

export const near = (a: Rgb, b: Rgb, tolerance = 10): boolean =>
	a.every((channel, index) => Math.abs(channel - (b[index] ?? 0)) <= tolerance);

/** The client y of every pixel of `color` at least `alpha` opaque down one column, from `top` to `bottom`. */
export const rowsOf = (
	canvas: HTMLCanvasElement,
	clientX: number,
	top: number,
	bottom: number,
	color: Rgb,
	alpha = 200,
): number[] => {
	const rows: number[] = [];
	for (let y = Math.ceil(top); y < bottom; y++) {
		const pixel = pixelAt(canvas, clientX, y);
		if (pixel.alpha > alpha && near(pixel.rgb, color)) {
			rows.push(y);
		}
	}
	return rows;
};

export const middle = (rows: readonly number[]): number =>
	rows.length === 0
		? Number.NaN
		: ((rows[0] as number) + (rows[rows.length - 1] as number)) / 2;

const nextFrame = (): Promise<void> =>
	new Promise((resolve) => requestAnimationFrame(() => resolve()));

/** Two frames: one for effects and microtasks to land, one for the canvas to paint. */
export const settle = async (): Promise<void> => {
	await nextFrame();
	await nextFrame();
};
