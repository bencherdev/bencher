import "../plot/plot.css";
import "./plots.css";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type { JsonPlot } from "../../types/bencher";
import { ApiError } from "../api";
import { fakeApi, mount } from "../settings/testing";
import PinDialog from "./PinDialog";
import { testTile } from "./testing";

const DAY = 86_400;

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
});

const order = [
	{ uuid: "plot-a", title: "Alpha" },
	{ uuid: "plot-1", title: "blake3, Latency on main" },
	{ uuid: "plot-c", title: "Gamma" },
	{ uuid: "plot-d", title: "Delta" },
];

/**
 * The dialog over pin `plot-1`; every plot query answers with its lines, and is recorded.
 * The order's read waits for `release` when `held`.
 */
const open = (
	props: {
		order?: typeof order;
		held?: boolean;
		refused?: number;
		pinned?: Partial<JsonPlot>;
	} = { order },
) => {
	let refusals = props.refused ?? 0;
	const tile = testTile();
	const plot = { ...tile.plot, ...props.pinned };
	const { perf } = tile;
	const reads: URL[] = [];
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const { api, sent } = fakeApi(
		async ({ body }) => ({ ...plot, ...(body as Partial<JsonPlot>) }),
		async (path: string) => {
			const url = new URL(path, "http://api.test");
			reads.push(url);
			if (
				props.held &&
				url.pathname.endsWith("/v0/console/projects/hashbrown/plots")
			) {
				await held;
			}
			if (
				refusals > 0 &&
				url.pathname.endsWith("/v0/console/projects/hashbrown/plots")
			) {
				refusals -= 1;
				throw new ApiError(403, "forbidden", "refused");
			}
			return {
				data: url.pathname.endsWith("/v0/console/projects/hashbrown/plots")
					? { order, plots: [] }
					: perf,
				headers: new Headers(),
			} as never;
		},
	);
	const saved: { plot: JsonPlot; order: string[] }[] = [];
	const closed: boolean[] = [];
	dispose = mount(
		() => (
			<PinDialog
				plot={plot}
				{...(props.order ? { order: props.order } : {})}
				onClose={() => closed.push(true)}
				onSaved={(next, nextOrder) =>
					saved.push({ plot: next, order: nextOrder })
				}
			/>
		),
		{ api },
	).dispose;
	const dialog = page.getByRole("dialog", { name: "Edit pin" });
	return { dialog, sent, reads, saved, closed, release };
};

const orderReads = (reads: URL[]) =>
	reads.filter(({ pathname }) =>
		pathname.endsWith("/v0/console/projects/hashbrown/plots"),
	);

const windowDays = (url: URL) =>
	(Number(url.searchParams.get("end_time")) -
		Number(url.searchParams.get("start_time"))) /
	(DAY * 1000);

// Kills an empty value shown bare in the saved query, which reads as no value.
test("the saved query shows an empty value as quotes", async () => {
	const { dialog } = open({ order, pinned: { parameters: [{ env: "" }] } });
	await expect.element(dialog.getByText(/ · env="" · /)).toBeVisible();
});

// Kills a custom window of the wrong length, a preview that stays at the saved
// window, a window the save leaves out, and a move sent when the pin stays put.
test("a custom window in days or weeks draws the preview and saves", async () => {
	const { dialog, sent, reads, saved } = open();
	await dialog.getByRole("radio", { name: "Custom" }).click();
	const count = dialog.getByRole("spinbutton", {
		name: "How many days or weeks",
	});
	await expect.element(count).toHaveValue(4);
	await expect
		.element(dialog.getByRole("combobox", { name: "Days or weeks" }))
		.toHaveValue("weeks");

	await count.fill("10");
	await userEvent.keyboard("{Tab}");
	await dialog
		.getByRole("combobox", { name: "Days or weeks" })
		.selectOptions("days");
	await expect.poll(() => reads.map(windowDays)).toContain(10);
	await expect
		.element(dialog.getByText("10 days", { exact: true }))
		.toBeVisible();

	await dialog.getByRole("radio", { name: "Custom" }).click();
	await count.fill("6");
	await userEvent.keyboard("{Tab}");
	await dialog
		.getByRole("combobox", { name: "Days or weeks" })
		.selectOptions("weeks");
	await expect.poll(() => reads.map(windowDays)).toContain(42);

	await dialog.getByRole("button", { name: "Save pin" }).click();
	expect(sent).toEqual([
		{
			method: "PATCH",
			path: "/v0/projects/hashbrown/plots/plot-1",
			body: { title: "blake3, Latency on main", window: 42 * DAY },
		},
	]);
	await expect.poll(() => saved.length).toBe(1);
	expect(saved[0]?.order).toEqual(order.map(({ uuid }) => uuid));
});

// Kills an after that lands before its plot or off by one, a title the save
// drops, and a dialog that asks for an order its caller gave it.
test("the position and the title save in one request", async () => {
	const { dialog, sent, saved, reads } = open();
	await dialog.getByRole("textbox", { name: "Title" }).fill("Renamed");
	await dialog.getByRole("radio", { name: "After" }).click();
	await dialog
		.getByRole("combobox", { name: "After which plot" })
		.selectOptions("Delta");
	await dialog.getByRole("button", { name: "Save pin" }).click();
	expect(sent.map(({ body }) => body)).toEqual([
		{ title: "Renamed", window: 28 * DAY, index: 3 },
	]);
	await expect
		.poll(() => saved[0]?.order)
		.toEqual(["plot-a", "plot-c", "plot-d", "plot-1"]);
	expect(orderReads(reads)).toEqual([]);
});

// Kills a top or bottom that sends the pin anywhere else.
test("top and bottom place the pin at either end", async () => {
	for (const [position, index] of [
		["Top", 0],
		["Bottom", 3],
	] as const) {
		const { dialog, sent } = open();
		await dialog.getByRole("radio", { name: position }).click();
		await dialog.getByRole("button", { name: "Save pin" }).click();
		expect(sent.map(({ body }) => (body as { index?: number }).index)).toEqual([
			index,
		]);
		dispose?.();
		dispose = undefined;
	}
});

// Kills a dialog opened without the other pins that cannot place the pin after one.
test("without the order, the dialog asks for it", async () => {
	const { dialog, reads } = open({});
	await dialog.getByRole("radio", { name: "After" }).click();
	await expect
		.element(dialog.getByRole("option", { name: "Gamma" }))
		.toBeInTheDocument();
	expect(
		orderReads(reads).map(({ searchParams }) => searchParams.get("per_page")),
	).toEqual(["0"]);
});

// Kills a position chosen before the order arrives, which the save then drops.
test("the position waits for the order", async () => {
	const { dialog, release } = open({ held: true });
	const bottom = dialog.getByRole("radio", { name: "Bottom" });
	await expect.element(bottom).toBeDisabled();
	await expect
		.element(dialog.getByRole("combobox", { name: "After which plot" }))
		.toBeDisabled();
	release();
	await expect.element(bottom).toBeEnabled();
});

// Kills a title counted in characters, which the API refuses past 64 bytes,
// and a disabled Save that does not say why.
test("a title over 64 bytes cannot save and says so", async () => {
	const { dialog } = open();
	const title = dialog.getByRole("textbox", { name: "Title" });
	const save = dialog.getByRole("button", { name: "Save pin" });
	await title.fill("€".repeat(21));
	await expect.element(save).toBeEnabled();
	await title.fill("€".repeat(22));
	await expect.element(save).toBeDisabled();
	await expect.element(title).toHaveAccessibleDescription("66 of 64 bytes");
});

// Kills a Save that sends a title the API refuses, and a Cancel that saves.
test("an empty title cannot save, and Cancel sends nothing", async () => {
	const { dialog, sent, closed } = open();
	await dialog.getByRole("textbox", { name: "Title" }).fill("   ");
	await expect
		.element(dialog.getByRole("button", { name: "Save pin" }))
		.toBeDisabled();
	await dialog.getByRole("button", { name: "Cancel" }).click();
	expect(sent).toEqual([]);
	expect(closed).toEqual([true]);
});

// Kills a refused order read that leaves the position reading as on its way
// for good, with no way to ask again but closing the dialog.
test("a refused order read says the order did not load, and Retry reads it", async () => {
	const { dialog, reads } = open({ refused: 1 });
	const failed = dialog.getByRole("alert");
	await expect
		.element(failed)
		.toHaveTextContent("The order of the pinned plots did not load");
	await expect
		.element(dialog.getByRole("radio", { name: "Top" }))
		.toBeDisabled();
	await failed.getByRole("button", { name: "Retry" }).click();
	await expect.element(dialog.getByText("4 of 64 pinned")).toBeVisible();
	await expect
		.element(dialog.getByRole("radio", { name: "Top" }))
		.toBeEnabled();
	expect(orderReads(reads)).toHaveLength(2);
});
