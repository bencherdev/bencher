import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "./explore.css";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import { blankQuery } from "../query/query";
import Controls from "./Controls";

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

// Kills a control's label left at the end of a line while its control wraps
// to the next.
for (const width of [320, 390, 768] as const) {
	test(`each view control's label wraps with its control at ${width} px`, async () => {
		await page.viewport(width, 700);
		const root = document.createElement("div");
		document.body.append(root);
		dispose = render(
			() => (
				<main class="page">
					<section class="ex-qp">
						<Controls
							query={{ ...blankQuery(), measures: ["a", "b"] }}
							total={undefined}
							now={() => 0}
							onQuery={() => {}}
						/>
					</section>
				</main>
			),
			root,
		);
		const groups = page.getByRole("radiogroup").elements();
		expect(groups).toHaveLength(4);
		for (const group of groups) {
			const label = group.previousElementSibling as Element;
			expect(label.classList.contains("eyebrow")).toBe(true);
			expect(
				label.getBoundingClientRect().bottom,
				group.ariaLabel ?? "",
			).toBeGreaterThan(group.getBoundingClientRect().top);
		}
		expect(document.documentElement.scrollWidth).toBe(width);
	});
}
