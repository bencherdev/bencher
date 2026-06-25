// One tick label per ~80px of plot width, matching the default `tickSpacing`
// that Observable Plot uses to thin the ticks on continuous x-axis scales.
const TICK_SPACING = 80;

// Explicit tick values for the version number x-axis.
// The version axis is a point scale, and Observable Plot renders one tick per
// domain value on a point scale, so the labels overlap once there are more
// versions than fit the width. Thin them to an even stride instead.
export const get_x_ticks = (
	versions: (number | undefined)[],
	width: number,
): number[] => {
	const distinct = [
		...new Set(
			versions.filter(
				(version): version is number =>
					typeof version === "number" && Number.isFinite(version),
			),
		),
	].sort((a, b) => a - b);
	const maxTicks = Math.max(2, Math.floor(width / TICK_SPACING));
	if (distinct.length <= maxTicks) {
		return distinct;
	}
	const step = Math.ceil(distinct.length / maxTicks);
	return distinct.filter((_, index) => index % step === 0);
};
