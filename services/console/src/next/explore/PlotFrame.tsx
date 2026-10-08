import { For, Show } from "solid-js";
import { useNarrow } from "../plot/narrow";
import type { PlotLayout } from "../plot/types";

/**
 * The plot's frame while its code loads, or once its query failed: as large as
 * the plot holds itself before its data arrives, so nothing moves when it does.
 */
const PlotFrame = (props: {
	reserve: { lines: number; measures: number };
	layout: PlotLayout;
	/** The plot is on its way, rather than refused. */
	loading: boolean;
}) => {
	const narrow = useNarrow();
	// The plot's own rule, kept here so its charting library stays in its chunk.
	const drawn = () => {
		const { measures } = props.reserve;
		if (measures <= 1) {
			return "single";
		}
		return measures === 2 && props.layout === "dual" ? "dual" : "stacked";
	};
	const panels = () => (drawn() === "stacked" ? props.reserve.measures : 1);
	const entries = () => Math.min(props.reserve.lines, narrow() ? 6 : 12);
	return (
		<figure
			class="pl"
			data-size="full"
			data-layout={drawn()}
			aria-label={plural(props.reserve.lines)}
			aria-hidden={props.loading ? undefined : "true"}
		>
			<div class="pl-head" />
			<div class="pl-body">
				<For each={Array.from({ length: panels() })}>
					{() => (
						<div class="pl-panel">
							<div class="pl-axes" />
							<div class="pl-area">
								<Show when={props.loading}>
									<div
										class="pl-skel"
										role="img"
										aria-label="Loading the plot"
									/>
								</Show>
							</div>
						</div>
					)}
				</For>
			</div>
			<div class="pl-key" aria-hidden="true">
				<div class="pl-khead">
					<span class="pl-skel-text" />
				</div>
				<div class="pl-kgrid">
					<For each={Array.from({ length: entries() })}>
						{() => (
							<div class="pl-ke">
								<span class="pl-ktog">
									<span class="pl-skel-text" />
								</span>
								<span class="pl-kfoc" />
							</div>
						)}
					</For>
				</div>
				<Show when={props.reserve.lines > entries()}>
					<span class="pl-kmore" />
				</Show>
			</div>
		</figure>
	);
};

export default PlotFrame;

const plural = (count: number) => `${count} ${count === 1 ? "line" : "lines"}`;
