import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Icon from "@bencherdev/ui/Icon";
import Segmented from "@bencherdev/ui/Segmented";
import TextInput from "@bencherdev/ui/TextInput";
import Truncate from "@bencherdev/ui/Truncate";
import { For, Show, createMemo, createSignal, createUniqueId } from "solid-js";
import {
	type JsonConsolePerf,
	type JsonConsolePlotTitle,
	type JsonPlot,
	PlotLayout,
	XAxis,
	YAxis,
} from "../../types/bencher";
import { queryFromPlot } from "../explore/pinned";
import { perfQuery } from "../explore/queries";
import {
	PRESETS,
	type RollingUnit,
	rollingName,
	rollingParts,
	rollingSeconds,
} from "../explore/window";
import { useProject } from "../project";
import { useQueryResult } from "../query";
import { type Position, placed, positionIndex, positionOf } from "./order";
import { PLOTS_LIMIT } from "./pins";
import { plotsQuery } from "./queries";
import Tile, { drawingOf, listed } from "./Tile";

type Choice = (typeof PRESETS)[number]["value"] | "custom";

const CHOICES: { value: Choice; label: string }[] = [
	...PRESETS.map(({ value }) => ({ value, label: value })),
	{ value: "custom", label: "Custom" },
];

const TITLE_BYTES = 64;
const encoder = new TextEncoder();

const PinDialog = (props: {
	plot: JsonPlot;
	/** Every pin in order, when the caller holds them; else the dialog asks. */
	order?: readonly JsonConsolePlotTitle[];
	/** Start in the title, to rename the pin. */
	rename?: boolean;
	onClose: () => void;
	onSaved: (plot: JsonPlot, order: string[]) => void;
}) => {
	const { api, slug } = useProject();
	const heading = createUniqueId();
	const titleId = createUniqueId();
	const titleHelp = createUniqueId();
	const windowId = createUniqueId();
	const uuid = () => props.plot.uuid;

	const [title, setTitle] = createSignal(props.plot.title ?? "");
	const saved = rollingParts(props.plot.window);
	const [choice, setChoice] = createSignal<Choice>(
		PRESETS.find(({ seconds }) => seconds === props.plot.window)?.value ??
			"custom",
	);
	const [count, setCount] = createSignal(saved.count);
	const [unit, setUnit] = createSignal<RollingUnit>(saved.unit);
	const seconds = () => {
		const chosen = choice();
		return (
			PRESETS.find(({ value }) => value === chosen)?.seconds ??
			rollingSeconds(count(), unit())
		);
	};
	const pick = (next: Choice) => {
		if (next === "custom" && choice() !== "custom") {
			const parts = rollingParts(seconds());
			setCount(parts.count);
			setUnit(parts.unit);
		}
		setChoice(next);
	};

	const asked = useQueryResult(() => ({
		...plotsQuery(api, slug(), undefined, { page: 1, perPage: 0 }),
		enabled: props.order === undefined,
	}));
	const known = () => props.order ?? asked().data?.order;
	const order = () => known() ?? [];
	const uuids = () => order().map((plot) => plot.uuid);
	const others = () => order().filter((plot) => plot.uuid !== uuid());
	const [position, setPosition] = createSignal<Position>();
	const placing = () => position() ?? positionOf(uuids(), uuid());
	const after = () => {
		const current = placing();
		return current.at === "after" ? current.plot : others()[0]?.uuid;
	};

	const preview = useQueryResult(() => ({
		...perfQuery(api, slug(), {
			...queryFromPlot(props.plot),
			window: { seconds: seconds() },
		}),
		placeholderData: (previous: JsonConsolePerf | undefined) => previous,
	}));
	// The tile follows the answer alone, so a keystroke in the title redraws nothing.
	const tile = createMemo(() => {
		const perf = preview().data;
		return perf && { plot: props.plot, perf };
	});

	const titled = () => title().trim();
	const bytes = () => encoder.encode(titled()).length;
	const fits = () => titled().length > 0 && bytes() <= TITLE_BYTES;
	const [saving, setSaving] = createSignal(false);
	const [failure, setFailure] = createSignal<string>();
	const save = async () => {
		const current = uuids();
		const index = positionIndex(current, uuid(), placing());
		const moved = current.includes(uuid()) && index !== current.indexOf(uuid());
		setSaving(true);
		setFailure(undefined);
		try {
			const { data } = await api.send<JsonPlot>(
				"PATCH",
				`/v0/projects/${encodeURIComponent(slug())}/plots/${encodeURIComponent(uuid())}`,
				{ title: titled(), window: seconds(), ...(moved ? { index } : {}) },
			);
			props.onSaved(data, moved ? placed(current, uuid(), index) : current);
		} catch {
			setFailure("Bencher did not save the pin");
		} finally {
			setSaving(false);
		}
	};

	return (
		<Dialog
			aria-labelledby={heading}
			data-size="wide"
			onDismiss={props.onClose}
		>
			<div class="ui-dialog-head">
				<h2 id={heading}>Edit pin</h2>
				<Button
					variant="ghost"
					iconOnly
					aria-label="Close without saving"
					onClick={props.onClose}
				>
					<Icon name="close" />
				</Button>
			</div>
			<div class="ui-dialog-body plots-dbody">
				<div class="plots-dprev">
					<span class="flab" aria-hidden="true">
						Preview on Plots
					</span>
					<Tile
						title={titled() || "Untitled"}
						tile={tile()}
						window={rollingName(seconds())}
						class="plots-preview"
					/>
				</div>
				<div class="plots-dfields">
					<div class="field">
						<label class="flab" for={titleId}>
							Title
						</label>
						<TextInput
							id={titleId}
							value={title()}
							autofocus={props.rename}
							aria-describedby={titleHelp}
							aria-invalid={bytes() > TITLE_BYTES}
							onInput={(event) => setTitle(event.currentTarget.value)}
						/>
						<span id={titleHelp} class="fhelp">
							{bytes() > TITLE_BYTES
								? `${bytes()} of ${TITLE_BYTES} bytes`
								: ""}
						</span>
					</div>
					<div class="field">
						<span class="flab" id={windowId}>
							Window
						</span>
						<div class="plots-drow">
							<Segmented
								name="pin-window"
								aria-labelledby={windowId}
								options={CHOICES}
								value={choice()}
								onChange={pick}
							/>
							<Show when={choice() === "custom"}>
								<span class="sm text2">the last</span>
								<TextInput
									type="number"
									class="plots-count"
									min={1}
									max={366}
									aria-label="How many days or weeks"
									value={count()}
									onChange={(event) =>
										setCount(Number(event.currentTarget.value))
									}
								/>
								<span class="ui-select-wrap">
									<select
										class="ui-select"
										aria-label="Days or weeks"
										value={unit()}
										onChange={(event) =>
											setUnit(event.currentTarget.value as RollingUnit)
										}
									>
										<option value="days">days</option>
										<option value="weeks">weeks</option>
									</select>
								</span>
							</Show>
						</div>
					</div>
					<fieldset class="fset">
						<legend class="flab">Position on Plots</legend>
						<div class="radios">
							<label class="radio">
								<input
									type="radio"
									name="pin-position"
									checked={placing().at === "top"}
									disabled={known() === undefined}
									onChange={() => setPosition({ at: "top" })}
								/>
								Top
							</label>
							<label class="radio">
								<input
									type="radio"
									name="pin-position"
									checked={placing().at === "bottom"}
									disabled={known() === undefined}
									onChange={() => setPosition({ at: "bottom" })}
								/>
								Bottom
							</label>
							<div class="plots-drow">
								<label class="radio">
									<input
										type="radio"
										name="pin-position"
										checked={placing().at === "after"}
										disabled={others().length === 0}
										onChange={() => {
											const plot = after();
											if (plot !== undefined) {
												setPosition({ at: "after", plot });
											}
										}}
									/>
									After
								</label>
								<span class="ui-select-wrap">
									<select
										class="ui-select"
										aria-label="After which plot"
										disabled={others().length === 0}
										value={after() ?? ""}
										onChange={(event) =>
											setPosition({
												at: "after",
												plot: event.currentTarget.value,
											})
										}
									>
										<For each={others()}>
											{(plot) => (
												<option value={plot.uuid}>
													{plot.title ?? "Untitled"}
												</option>
											)}
										</For>
									</select>
								</span>
							</div>
						</div>
						<Show
							when={known() === undefined && asked().isError}
							fallback={
								<span class="fhelp">
									{known() === undefined
										? ""
										: `${order().length} of ${PLOTS_LIMIT} pinned`}
								</span>
							}
						>
							<span class="fhelp plots-dorder" role="alert">
								The order of the pinned plots did not load
								<Button
									size="sm"
									disabled={asked().isFetching}
									onClick={() => void asked().refetch()}
								>
									Retry
								</Button>
							</span>
						</Show>
					</fieldset>
					<div class="field">
						<span class="flab">Saved with it</span>
						<SavedWith plot={props.plot} tile={tile()} />
					</div>
				</div>
			</div>
			<Show when={failure()}>
				<p class="ferror plots-dfail" role="alert">
					{failure()}
				</p>
			</Show>
			<div class="ui-dialog-foot">
				<span class="spacer" />
				<Button onClick={props.onClose}>Cancel</Button>
				<Button
					variant="primary"
					disabled={!fits() || saving()}
					onClick={() => void save()}
				>
					Save pin
				</Button>
			</div>
		</Dialog>
	);
};

export default PinDialog;

const SavedWith = (props: {
	plot: JsonPlot;
	tile: Parameters<typeof drawingOf>[0] | undefined;
}) => {
	const named = (names: readonly { name: string }[] | undefined) =>
		listed((names ?? []).map(({ name }) => name));
	const query = () => {
		const perf = props.tile?.perf;
		const sets = props.plot.parameters ?? [];
		return [
			named(perf?.branches),
			named(perf?.testbeds),
			named(perf?.benchmarks),
			sets.length === 0
				? "every variant"
				: sets
						.map((set) =>
							Object.entries(set)
								.map(([key, value]) => `${key}=${String(value)}`)
								.join(" "),
						)
						.join(", "),
			named(perf?.measures),
			props.plot.metrics?.join(", ") ?? "every metric",
		].join(" · ");
	};
	const lines = (count: number) => (count === 1 ? "1 line" : `${count} lines`);
	const focus = () => {
		const tile = props.tile;
		const line =
			tile && drawingOf(tile).lines.find(({ id }) => id === props.plot.focus);
		if (!line) {
			return props.plot.focus === undefined ? "none" : "a line not drawn now";
		}
		const tags = Object.entries(line.parameters).map(
			([key, value]) => `${key}=${String(value)}`,
		);
		return [line.benchmark, ...tags].join(" ");
	};
	const view = () =>
		[
			props.plot.x_axis === XAxis.Version ? "version" : "report date",
			`${props.plot.y_axis === YAxis.Auto ? "auto" : props.plot.y_axis} y scale`,
			...(props.plot.layout === PlotLayout.Stacked ? ["stacked"] : []),
		].join(", ");
	return (
		<dl class="plots-kv">
			<div class="kv">
				<dt>query</dt>
				<dd>{query()}</dd>
			</div>
			<div class="kv">
				<dt>hidden lines</dt>
				<dd>
					{props.plot.hidden?.length ? lines(props.plot.hidden.length) : "none"}
				</dd>
			</div>
			<div class="kv">
				<dt>focused line</dt>
				<dd>
					<Truncate text={focus()} tail reveal />
				</dd>
			</div>
			<div class="kv">
				<dt>view</dt>
				<dd>{view()}</dd>
			</div>
		</dl>
	);
};
