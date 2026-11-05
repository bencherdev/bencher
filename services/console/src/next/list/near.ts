import { createEffect, on, onCleanup, onMount } from "solid-js";

/**
 * For a list drawn whole as it loads: `load` the next page while the list's
 * end is within a screen of the window's bottom, checked on scroll, on
 * resize, and as rows arrive.
 */
export const loadNearEnd = (
	list: () => Element | undefined,
	load: () => void,
	rows: () => number,
) => {
	let frame: number | undefined;
	const check = () => {
		frame = undefined;
		const end = list()?.getBoundingClientRect().bottom;
		if (end !== undefined && end < window.innerHeight * 2) {
			load();
		}
	};
	const schedule = () => {
		frame ??= requestAnimationFrame(check);
	};
	onMount(() => {
		window.addEventListener("scroll", schedule, { passive: true });
		window.addEventListener("resize", schedule, { passive: true });
		onCleanup(() => {
			window.removeEventListener("scroll", schedule);
			window.removeEventListener("resize", schedule);
			if (frame !== undefined) {
				cancelAnimationFrame(frame);
			}
		});
	});
	createEffect(on(rows, schedule));
};
