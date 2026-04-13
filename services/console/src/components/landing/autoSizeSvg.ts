/**
 * Auto-size an SVG's viewBox height to match the actual content height of
 * the HTML inside its first `<foreignObject>`. The SVG's viewBox width is
 * preserved from the initial attribute (defaults to 1040 if unset).
 *
 * Used by landing mocks where the HTML content inside the SVG can reflow
 * (e.g. `<details>` expanding, responsive text wrap) and the wrapping SVG
 * needs to grow/shrink to avoid clipping or leaving empty space.
 */
export function autoSizeSvgForeignObject(selector: string) {
	const svgs = document.querySelectorAll<SVGSVGElement>(selector);
	for (const svg of svgs) {
		const fo = svg.querySelector("foreignObject");
		const content = fo?.firstElementChild as HTMLElement | null;
		if (!fo || !content) continue;

		const initialViewBox = svg.getAttribute("viewBox") ?? "0 0 1040 0";
		const width = initialViewBox.split(" ")[2] ?? "1040";

		const update = () => {
			const h = Math.ceil(content.offsetHeight);
			if (!h) return;
			svg.setAttribute("viewBox", `0 0 ${width} ${h}`);
			fo.setAttribute("height", String(h));
		};

		requestAnimationFrame(update);
		new ResizeObserver(update).observe(content);
	}
}
