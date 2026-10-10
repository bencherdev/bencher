/**
 * Replace the static page with the app's render of it, keeping the reader's
 * focus on the same control and each animation on the clock it started on.
 */
export const handOff = (mount: HTMLElement, paint: () => void) => {
	const focused = document.activeElement;
	const control =
		focused instanceof HTMLElement && mount.contains(focused)
			? identity(focused)
			: undefined;
	const started = startTimes(mount);

	mount.replaceChildren();
	paint();

	if (control) {
		for (const element of mount.querySelectorAll<HTMLElement>("a, button")) {
			if (identity(element) === control) {
				element.focus({ preventScroll: true });
				break;
			}
		}
	}
	for (const animation of mount.getAnimations({ subtree: true })) {
		const startTime =
			animation instanceof CSSAnimation && started.get(animation.animationName);
		if (typeof startTime === "number") {
			animation.startTime = startTime;
		}
	}
};

// The bell and the Alerts tab share a link, so the class tells them apart.
const identity = (element: Element) =>
	[element.tagName, element.className, element.getAttribute("href")].join(" ");

/** The earliest start of each CSS animation running in `mount`. */
const startTimes = (mount: HTMLElement) => {
	const started = new Map<string, number>();
	for (const animation of mount.getAnimations({ subtree: true })) {
		const { startTime } = animation;
		if (animation instanceof CSSAnimation && typeof startTime === "number") {
			const earliest = started.get(animation.animationName);
			if (earliest === undefined || startTime < earliest) {
				started.set(animation.animationName, startTime);
			}
		}
	}
	return started;
};
