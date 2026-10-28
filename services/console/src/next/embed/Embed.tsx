import Skeleton from "@bencherdev/ui/Skeleton";
import { type JSX, Match, Show, Switch, createMemo } from "solid-js";
import type { JsonConsolePerf } from "../../types/bencher";
import { plotData } from "../explore/data";
import { measuresLayout } from "../explore/layout";
import PlotFrame from "../explore/PlotFrame";
import { drawable } from "../explore/request";
import { publicPlotPath } from "../public/query";
import { lineVisible, settleVisibility } from "../query/line";
import { type ExploreQuery, encodeQuery } from "../query/query";

export type PlotView = typeof import("../plot/Plot").default;

export interface EmbedProps {
	slug: string;
	/** The project's name, once it is read. */
	name?: string | undefined;
	/** The project's read failed, so the line names it by its slug. */
	nameless?: boolean | undefined;
	query: ExploreQuery;
	/** The plot, or a project the API refused, which may not exist. */
	view: "plot" | "locked";
	answer?: JsonConsolePerf | undefined;
	/** Why the plot did not arrive. */
	refused?: string | undefined;
	plot?: PlotView | undefined;
	onQuery?: (query: ExploreQuery) => void;
	/** Where the line and the readout link to; without them, the line opens the public plot and the readout has no links. */
	links?: EmbedLinks | undefined;
	/** The wordmark's images, in place of the site's own. */
	wordmarks?: { dark: string; light: string } | undefined;
	/** Controls at the end of the line. */
	actions?: JSX.Element;
}

export interface EmbedLinks {
	plot: string;
	report: (uuid: string) => string;
	alert: (uuid: string) => string;
}

/** A public project's plot and key for a frame on another site, under a line that opens its public plot. */
const Embed = (props: EmbedProps) => {
	const href = () =>
		props.links?.plot ??
		`${publicPlotPath(props.slug)}${encodeQuery(props.query)}`;
	const edit = (next: ExploreQuery) => props.onQuery?.(next);
	const data = createMemo(() => props.answer && plotData(props.answer));
	const drawn = createMemo(() => data()?.lines.map(({ id }) => id) ?? []);
	const settled = () => settleVisibility(props.query, drawn());
	const layout = () =>
		measuresLayout(props.query.measures.length, props.query.layout).layout;
	const reserve = createMemo(
		() => ({
			lines: props.query.only?.length ?? 1,
			measures: Math.max(1, props.query.measures.length),
		}),
		undefined,
		{ equals: (a, b) => a.lines === b.lines && a.measures === b.measures },
	);
	const name = () =>
		props.name ??
		(props.nameless || props.view === "locked" ? props.slug : undefined);
	return (
		<main class="em">
			<div class="em-line">
				<h1 class="em-name">
					<Show
						when={name()}
						fallback={<Skeleton size="text" class="em-skel" />}
					>
						{(text) => (
							<a href={href()} target="_blank" rel="noopener">
								{text()}
							</a>
						)}
					</Show>
				</h1>
				<a
					class="brand"
					href={href()}
					target="_blank"
					rel="noopener"
					aria-label="Bencher"
				>
					<img src={props.wordmarks?.dark ?? "/wordmark-dark.svg"} alt="" />
					<img src={props.wordmarks?.light ?? "/wordmark-light.svg"} alt="" />
				</a>
				{props.actions}
			</div>
			<Switch
				fallback={
					<section class="em-plot" aria-label="Plot">
						<Show when={props.refused}>
							<p class="em-failed" role="alert">
								{props.refused}
							</p>
						</Show>
						<Show
							when={
								(props.answer !== undefined || !props.refused) && props.plot
							}
							fallback={
								<PlotFrame
									reserve={reserve()}
									layout={layout()}
									loading={!props.refused}
								/>
							}
						>
							{(View) => {
								const Loaded = View();
								return (
									<Loaded
										data={data()}
										hidden={drawn().filter(
											(id) => !lineVisible(props.query, id),
										)}
										focused={props.query.focus ?? null}
										onHiddenChange={(hide) => edit({ ...settled(), hide })}
										onFocusChange={(focus) => {
											const { focus: _focus, ...rest } = settled();
											edit(focus === null ? rest : { ...rest, focus });
										}}
										xAxis={props.query.xAxis}
										scale={props.query.yScale}
										layout={layout()}
										reserve={reserve()}
										{...(props.links && { reportHref: props.links.report })}
										alerts={props.links && { href: props.links.alert }}
										note={
											props.answer?.window.clamped
												? "Cut to 3 months"
												: undefined
										}
									/>
								);
							}}
						</Show>
					</section>
				}
			>
				<Match when={props.view === "locked"}>
					<p class="em-note">Private or not found</p>
				</Match>
				<Match when={!drawable(props.query)}>
					<p class="em-note">No lines</p>
				</Match>
			</Switch>
		</main>
	);
};

export default Embed;
