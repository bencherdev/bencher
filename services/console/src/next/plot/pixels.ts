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

export interface Drawn {
	canvas: HTMLCanvasElement;
	align: CanvasTextAlign;
	text: string;
}

/** Every label the plots draw while `act` runs. */
export const drawnDuring = async (
	act: () => Promise<void>,
): Promise<Drawn[]> => {
	const drawn: Drawn[] = [];
	const fillText = CanvasRenderingContext2D.prototype.fillText;
	CanvasRenderingContext2D.prototype.fillText = function (
		this: CanvasRenderingContext2D,
		...args: Parameters<typeof fillText>
	) {
		drawn.push({ canvas: this.canvas, align: this.textAlign, text: args[0] });
		return fillText.apply(this, args);
	};
	try {
		await act();
		await settle();
	} finally {
		CanvasRenderingContext2D.prototype.fillText = fillText;
	}
	return drawn;
};

/** The x axis labels drawn, each once. */
export const xLabels = (drawn: Drawn[]) => [
	...new Set(
		drawn.filter(({ align }) => align === "center").map(({ text }) => text),
	),
];

/** The values a y axis labels: the left one's labels align right, the right one's left. */
export const yValues = (
	drawn: Drawn[],
	side: "left" | "right",
	canvas?: HTMLCanvasElement,
) =>
	drawn
		.filter(
			(label) =>
				label.align === (side === "left" ? "right" : "left") &&
				(canvas === undefined || label.canvas === canvas),
		)
		.map(({ text }) => Number(text));

/** The canvas under a plot area. */
export const canvasOf = (over: HTMLElement) =>
	over.parentElement?.querySelector("canvas") as HTMLCanvasElement;
