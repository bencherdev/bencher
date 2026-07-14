import {
	type Accessor,
	Show,
	createMemo,
	createSignal,
	onCleanup,
	onMount,
} from "solid-js";
import type { PerfTab } from "../../../config/types";
import {
	type JsonAuthUser,
	type JsonPerfQuery,
	type JsonPlot,
	XAxis,
} from "../../../types/bencher";
import { timeToDateOnlyIso } from "../../../util/convert";
import { theme } from "../../navbar/theme/util";
import PerfFrame from "../perf/PerfFrame";
import FallbackPlot from "./FallbackPlot";
import { PLOT_PRELOAD_MARGIN, isNearViewport } from "./util";

export interface Props {
	children?: Element;
	isConsole: boolean;
	apiUrl: string;
	user: JsonAuthUser;
	project_slug: Accessor<undefined | string>;
	plot: JsonPlot;
	logo?: boolean;
	embed?: {
		logo?: boolean;
		title?: string;
		header?: boolean;
		key?: boolean;
	};
}

const PinnedFrame = (props: Props) => {
	const plotId = createMemo(() => props.plot?.uuid);

	const [isVisible, setIsVisible] = createSignal(false);

	onMount(() => {
		let observer: IntersectionObserver | undefined;
		let resizeObserver: ResizeObserver | undefined;
		let retryTimeout: ReturnType<typeof setTimeout> | undefined;
		const stopObserving = () => {
			clearTimeout(retryTimeout);
			observer?.disconnect();
			resizeObserver?.disconnect();
		};
		// Registered synchronously within the onMount owner so cleanup runs even
		// when the element lookup below has to retry asynchronously.
		onCleanup(stopObserving);

		const isTargetVisible = (target: HTMLElement) =>
			isNearViewport(target.getBoundingClientRect(), window.innerHeight);
		const reveal = () => {
			setIsVisible(true);
			// Once visible we never hide again, so stop watching.
			stopObserving();
		};

		const setupObserver = () => {
			const target = document.getElementById(plotId());
			if (!target) {
				// Retry if the target is not yet in the DOM
				retryTimeout = setTimeout(setupObserver, 1);
				return;
			}
			// If the plot is already on (or near) the screen at mount time, load it
			// immediately instead of relying on the IntersectionObserver's initial
			// callback, whose delivery timing is engine dependent and can be delayed
			// or missed for elements that are already in view and never subsequently
			// move, which left pinned plots stuck on the loading skeleton until a
			// manual refresh re-mounted them.
			if (isTargetVisible(target)) {
				reveal();
				return;
			}
			// Observe for the plot being scrolled into view later.
			observer = new IntersectionObserver(
				(entries) => {
					if (entries.some((entry) => entry.isIntersecting)) {
						reveal();
					}
				},
				{ rootMargin: `${PLOT_PRELOAD_MARGIN}px` },
			);
			observer.observe(target);
			// As sibling plots stream in their data, the layout shifts and can move
			// this plot into view without a scroll. The IntersectionObserver does not
			// reliably report that on an otherwise idle page, which left an in-view
			// plot stuck on the skeleton until the user scrolled or refreshed. So also
			// re-check the plot's position whenever the page layout changes. A
			// ResizeObserver fires only on real layout changes (not on a timer) and
			// delivers an initial callback, so a settled-but-mismeasured mount is
			// caught too.
			resizeObserver = new ResizeObserver(() => {
				if (isTargetVisible(target)) {
					reveal();
				}
			});
			resizeObserver.observe(document.body);
		};
		setupObserver();
	});

	const branchesIsEmpty = createMemo(
		() => (props.plot?.branches?.length ?? 0) === 0,
	);
	const testbedsIsEmpty = createMemo(
		() => (props.plot?.testbeds?.length ?? 0) === 0,
	);
	const benchmarksIsEmpty = createMemo(
		() => (props.plot?.benchmarks?.length ?? 0) === 0,
	);
	const measuresIsEmpty = createMemo(
		() => (props.plot?.measures?.length ?? 0) === 0,
	);
	const isPlotInit = createMemo(
		() =>
			branchesIsEmpty() ||
			testbedsIsEmpty() ||
			benchmarksIsEmpty() ||
			measuresIsEmpty(),
	);

	const start_time = createMemo(() => {
		const now = Date.now();
		const windowMillis = (props.plot?.window ?? 0) * 1_000;
		// start_time needs to be positive,
		// so if the window is too large, just start from the unix epoch
		return (windowMillis > now ? 0 : now - windowMillis).toString();
	});
	const end_time = createMemo(() => Date.now().toString());

	const perfQuery = createMemo(() => {
		return {
			branches: props.plot?.branches ?? [],
			heads: [],
			testbeds: props.plot?.testbeds ?? [],
			benchmarks: props.plot?.benchmarks ?? [],
			measures: props.plot?.measures ?? [],
			start_time: start_time(),
			end_time: end_time(),
		} as JsonPerfQuery;
	});

	const refresh = createMemo(() => 0);

	const measures = createMemo(() => props.plot?.measures ?? []);

	const start_date = createMemo(() => timeToDateOnlyIso(start_time()));
	const end_date = createMemo(() => timeToDateOnlyIso(end_time()));

	const [key, setKey] = createSignal(false);
	const x_axis = createMemo(() => props.plot?.x_axis ?? XAxis.DateTime);
	const clear = createMemo(() => false);

	const lower_value = createMemo(() => props.plot?.lower_value ?? false);
	const upper_value = createMemo(() => props.plot?.upper_value ?? false);
	const lower_boundary = createMemo(() => props.plot?.lower_boundary ?? false);
	const upper_boundary = createMemo(() => props.plot?.upper_boundary ?? false);

	const embed_logo = createMemo(() => props.embed?.logo === true);
	const embed_title = createMemo(
		() => props.embed?.title ?? props.plot?.title ?? "",
	);
	const embed_header = createMemo(() => props.embed?.header === true);
	const embed_key = createMemo(() => props.embed?.key === true);

	const handleVoid = (_void: string | PerfTab | boolean | XAxis | null) => {};

	return (
		<div id={plotId()}>
			<Show when={isVisible()} fallback={<FallbackPlot />}>
				<PerfFrame
					apiUrl={props.apiUrl}
					user={props.user}
					isConsole={props.isConsole}
					isEmbed={true}
					plotId={plotId()}
					theme={theme}
					project_slug={props.project_slug}
					measuresIsEmpty={measuresIsEmpty}
					branchesIsEmpty={branchesIsEmpty}
					testbedsIsEmpty={testbedsIsEmpty}
					benchmarksIsEmpty={benchmarksIsEmpty}
					isPlotInit={isPlotInit}
					perfQuery={perfQuery}
					refresh={refresh}
					measures={measures}
					start_date={start_date}
					end_date={end_date}
					key={key}
					x_axis={x_axis}
					clear={clear}
					lower_value={lower_value}
					upper_value={upper_value}
					lower_boundary={lower_boundary}
					upper_boundary={upper_boundary}
					embed_logo={embed_logo}
					embed_title={embed_title}
					embed_header={embed_header}
					embed_key={embed_key}
					handleMeasure={handleVoid}
					handleStartTime={handleVoid}
					handleEndTime={handleVoid}
					handleTab={handleVoid}
					handleKey={setKey}
					handleXAxis={handleVoid}
					handleClear={handleVoid}
					handleLowerValue={handleVoid}
					handleUpperValue={handleVoid}
					handleLowerBoundary={handleVoid}
					handleUpperBoundary={handleVoid}
				>
					{props.children}
				</PerfFrame>
			</Show>
		</div>
	);
};

export default PinnedFrame;
