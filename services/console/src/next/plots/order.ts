export type Move = "up" | "down" | "top" | "bottom";

/**
 * The index a move sends a plot to, which the API reads as its place among the
 * others; none when the plot cannot move that way.
 */
export const moveIndex = (
	order: readonly string[],
	plot: string,
	move: Move,
): number | undefined => {
	const at = order.indexOf(plot);
	const last = order.length - 1;
	const to = { up: at - 1, down: at + 1, top: 0, bottom: last }[move];
	return at < 0 || to < 0 || to > last || to === at ? undefined : to;
};

/** The order with `plot` at `index` among the others. */
export const placed = (
	order: readonly string[],
	plot: string,
	index: number,
): string[] => {
	const others = order.filter((uuid) => uuid !== plot);
	others.splice(Math.max(0, Math.min(others.length, index)), 0, plot);
	return others;
};

/** Where the pin dialog puts a pin. */
export type Position =
	| { at: "top" }
	| { at: "bottom" }
	| { at: "after"; plot: string };

/** The index a position sends `plot` to; after a plot no longer there is the bottom. */
export const positionIndex = (
	order: readonly string[],
	plot: string,
	position: Position,
): number => {
	const others = order.filter((uuid) => uuid !== plot);
	switch (position.at) {
		case "top":
			return 0;
		case "bottom":
			return others.length;
		case "after": {
			const at = others.indexOf(position.plot);
			return at < 0 ? others.length : at + 1;
		}
	}
};

/** The position a pin holds: the top, or after the plot above it. */
export const positionOf = (
	order: readonly string[],
	plot: string,
): Position => {
	const above = order[order.indexOf(plot) - 1];
	return above === undefined ? { at: "top" } : { at: "after", plot: above };
};
