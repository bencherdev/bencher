import type { Accessor } from "solid-js";
import type { JsonPlot } from "../../../types/bencher";
import { plotQueryString } from "./util";

const Pinned = (props: {
	plot: JsonPlot;
	index: Accessor<number>;
	refresh: () => JsonPlot[] | Promise<JsonPlot[]> | null | undefined;
}) => {
	return (
		<div id={props.plot?.uuid} class="box">
			<PinnedPlot plot={props.plot} />
			<PinnedSettings plot={props.plot} />
		</div>
	);
};

const PinnedPlot = (props: { plot: JsonPlot }) => {
	return (
		<iframe
			loading="lazy"
			src={`/perf/${props.plot?.project}/embed?embed_logo=false&embed_title=${
				props.plot?.title
			}&embed_header=false&embed_key=false&${plotQueryString(props.plot)}`}
			title={props.plot?.title ?? "Perf Plot"}
			width="100%"
			height="600px"
		/>
	);
};

const PinnedSettings = (props: { plot: JsonPlot }) => {
	return (
		<div class="buttons is-right">
			<a
				type="button"
				class="button is-small"
				title="View this Perf Plot"
				href={`/console/projects/${props.plot?.project}/perf?${plotQueryString(
					props.plot,
				)}`}
			>
				<span class="icon is-small">
					<i class="fas fa-external-link-alt" />
				</span>
			</a>
			<button type="button" class="button is-small">
				<span class="icon is-small">
					<i class="fas fa-cog" />
				</span>
			</button>
		</div>
	);
};

export default Pinned;
