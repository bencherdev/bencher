import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import Table from "@bencherdev/ui/Table";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

// Kills a fold keyed to the screen, or to one width for every table: a table
// folds by its own container's width, at its own fold width.
test.each([
	[400, 737, true],
	[1440, 737, false],
	[600, 500, false],
	[600, 700, true],
] as const)(
	"a Table in a %i px container on a 1440 px screen, folding at %i px, folds: %s",
	async (width, fold, folds) => {
		await page.viewport(1440, 900);
		const box = document.createElement("div");
		box.style.width = `${width}px`;
		document.body.append(box);
		const told: number[] = [];
		dispose = render(
			() => (
				<Table
					fold={fold}
					aria-label="Keys"
					onLines={(lines) => told.push(lines)}
				>
					<thead>
						<tr>
							<th scope="col">Name</th>
							<th scope="col">Created</th>
						</tr>
					</thead>
					<tbody>
						<tr>
							<td data-fold="l1">ci</td>
							<td data-fold="l2">Oct 28, 2026</td>
						</tr>
					</tbody>
				</Table>
			),
			box,
		);
		const head = box.querySelector("thead") as HTMLElement;
		expect(getComputedStyle(head).display === "none").toBe(folds);
		await expect.poll(() => told).toEqual([folds ? 2 : 1]);
	},
);

// Kills a track that lets a hidden header, placed absolutely, out past it, so
// a table wider than its track widens the page.
test("a table wider than its track keeps its hidden header inside it", async () => {
	await page.viewport(320, 640);
	dispose = render(
		() => (
			<Table aria-label="Keys">
				<thead>
					<tr>
						<th scope="col">Name</th>
						<th scope="col">
							<span class="sr-only">Actions</span>
						</th>
					</tr>
				</thead>
				<tbody>
					<tr>
						<td>{"a".repeat(200)}</td>
						<td>Revoke</td>
					</tr>
				</tbody>
			</Table>
		),
		document.body,
	);
	expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(320);
});

// Kills unfolded and folded rules that both hold at a fold width itself (the
// width rides in as a font size, a float), which draws a folded name cell 0px
// wide.
test.each([533, 667, 1145])(
	"at its fold width of %i px a table is folded and its name cell keeps its width",
	async (fold) => {
		await page.viewport(1440, 900);
		const box = document.createElement("div");
		document.body.append(box);
		dispose = render(
			() => (
				<Table fold={fold} aria-label="Runs">
					<thead>
						<tr>
							<th scope="col">Branch</th>
						</tr>
					</thead>
					<tbody>
						<tr>
							<td data-fold="l1" class="ui-table-name">
								main
							</td>
						</tr>
					</tbody>
				</Table>
			),
			box,
		);
		const scroll = box.querySelector(".ui-table-scroll") as HTMLElement;
		box.style.width = `${fold + scroll.offsetWidth - scroll.clientWidth}px`;
		expect(scroll.clientWidth).toBe(fold);
		const table = box.querySelector("table") as HTMLElement;
		const name = box.querySelector("td") as HTMLElement;
		expect(getComputedStyle(table).display).toBe("block");
		expect(getComputedStyle(name).maxWidth).toBe("none");
		expect(name.getBoundingClientRect().width).toBeGreaterThan(0);
	},
);
