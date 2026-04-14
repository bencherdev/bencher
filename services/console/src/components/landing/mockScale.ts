/**
 * Proportionally scale an HTML mock down to fit its container width.
 *
 * Structure expected:
 *
 *   <div class="wrap"><div class="scale">...</div></div>
 *
 * The `scale` element's CSS should declare:
 *
 *   transform-origin: 0 0;
 *   transform: scale(var(--mock-scale, 1));
 *
 * We set `--mock-scale` on the element and update the wrap's height so the
 * page flow matches the post-scale visual height. Using a CSS custom property
 * keeps the `transform-origin` and `transform` chain in the stylesheet,
 * which avoids accidental overrides from inline style timing or specificity.
 *
 * Works uniformly across Chrome, Safari, and Firefox for plain HTML content.
 */
export function scaleMock(selector: string) {
	const run = () => {
		const wraps = document.querySelectorAll<HTMLElement>(selector);
		for (const wrap of wraps) {
			const scale = wrap.firstElementChild as HTMLElement | null;
			if (!scale) continue;

			const update = () => {
				// Reset scale first so we measure the element's natural width.
				scale.style.setProperty("--mock-scale", "1");
				const scaleWidth = scale.offsetWidth;
				const wrapWidth = wrap.clientWidth;
				if (!scaleWidth || !wrapWidth) return;

				if (scaleWidth <= wrapWidth) {
					wrap.style.height = "";
					return; // already fits, no scaling needed
				}

				const ratio = wrapWidth / scaleWidth;
				scale.style.setProperty("--mock-scale", String(ratio));
				// Ceil so sub-pixel rounding never crops the scaled content's
				// bottom edge / border.
				wrap.style.height = `${Math.ceil(scale.offsetHeight * ratio)}px`;
			};

			update();
			new ResizeObserver(update).observe(wrap);
		}
	};

	if (document.readyState === "loading") {
		document.addEventListener("DOMContentLoaded", run);
	} else {
		run();
	}
}
