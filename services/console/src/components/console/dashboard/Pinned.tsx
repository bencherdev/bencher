import bencher_valid_init, { type InitOutput } from "bencher_valid";
import {
	Match,
	Show,
	Switch,
	createMemo,
	createResource,
	createSignal,
	type Accessor,
	type Resource,
} from "solid-js";
import type {
	JsonAuthUser,
	JsonPlot,
	JsonProject,
} from "../../../types/bencher";
import { plotQueryString } from "./util";
import DeleteButton from "../deck/hand/DeleteButton";
import type { Params } from "astro";
import DeckCard from "../deck/hand/card/DeckCard";
import { Card, Display } from "../../../config/types";
import { isAllowedProjectManage } from "../../../util/auth";
import { plotFields } from "../../../config/project/plot";
import FieldKind from "../../field/kind";
import { set } from "astro/zod";
import { httpPatch } from "../../../util/http";

enum PinnedState {
	Front = "front",
	Settings = "settings",
}

const Pinned = (props: {
	apiUrl: string;
	params: Params;
	user: JsonAuthUser;
	project: Resource<JsonProject>;
	isAllowed: Resource<boolean>;
	plot: JsonPlot;
	index: Accessor<number>;
	total: Accessor<number>;
	refresh: () => JsonPlot[] | Promise<JsonPlot[]> | null | undefined;
}) => {
	const [state, setState] = createSignal(PinnedState.Front);

	return (
		<div id={props.plot?.uuid} class="box">
			<Switch>
				<Match when={state() === PinnedState.Front}>
					<PinnedFront
						apiUrl={props.apiUrl}
						user={props.user}
						isAllowed={props.isAllowed}
						plot={props.plot}
						index={props.index}
						total={props.total}
						refresh={props.refresh}
						handleState={setState}
					/>
				</Match>
				<Match when={state() === PinnedState.Settings}>
					<PinnedSetting
						apiUrl={props.apiUrl}
						params={props.params}
						user={props.user}
						project={props.project}
						isAllowed={props.isAllowed}
						plot={props.plot}
						refresh={props.refresh}
						handleState={setState}
					/>
				</Match>
			</Switch>
		</div>
	);
};

const PinnedFront = (props: {
	apiUrl: string;
	user: JsonAuthUser;
	isAllowed: Resource<boolean>;
	plot: JsonPlot;
	index: Accessor<number>;
	total: Accessor<number>;
	refresh: () => JsonPlot[] | Promise<JsonPlot[]> | null | undefined;
	handleState: (state: PinnedState) => void;
}) => {
	return (
		<>
			<PinnedPlot plot={props.plot} />
			<PinnedButtons
				apiUrl={props.apiUrl}
				user={props.user}
				isAllowed={props.isAllowed}
				plot={props.plot}
				index={props.index}
				total={props.total}
				refresh={props.refresh}
				handleState={props.handleState}
			/>
		</>
	);
};

const PinnedPlot = (props: { plot: JsonPlot }) => {
	return (
		<iframe
			loading="lazy"
			src={`/perf/${props.plot?.project}/embed?embed_logo=false&embed_title=${
				props.plot?.title ?? ""
			}&embed_header=false&embed_key=false&${plotQueryString(props.plot)}`}
			title={props.plot?.title ?? "Perf Plot"}
			width="100%"
			height="600px"
		/>
	);
};

enum Rank {
	Down = 2,
	None = 0,
	Up = -1,
}

const PinnedButtons = (props: {
	apiUrl: string;
	user: JsonAuthUser;
	isAllowed: Resource<boolean>;
	plot: JsonPlot;
	index: Accessor<number>;
	total: Accessor<number>;
	refresh: () => JsonPlot[] | Promise<JsonPlot[]> | null | undefined;
	handleState: (state: PinnedState) => void;
}) => {
	const [rank, setRank] = createSignal(-1);

	const rankFetcher = createMemo(() => {
		return {
			token: props.user.token,
			plot: props.plot,
			rank: rank(),
		};
	});
	const patchRank = async (fetcher: {
		token: string;
		plot: JsonPlot;
		rank: number;
	}) => {
		if (fetcher.rank < 0) {
			return fetcher.plot;
		}
		setRank(-1);
		const path = `/v0/projects/${fetcher.plot?.project}/plots/${fetcher.plot?.uuid}`;
		const data = {
			rank: fetcher.rank,
		};
		return await httpPatch(props.apiUrl, path, fetcher.token, data)
			.then((resp) => {
				props.refresh();
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				return;
			});
	};
	const [_rank] = createResource<JsonPlot>(rankFetcher, patchRank);

	return (
		<nav class="level">
			<div class="level-left">
				<Show when={props.isAllowed()}>
					<div class="field has-addons">
						<p class="control">
							<button
								type="button"
								class="button is-small"
								title="Move plot to bottom"
								disabled={props.index() === props.total() - 1}
								onClick={(e) => {
									e.preventDefault();
									setRank(props.total() + 1);
								}}
							>
								<span class="icon is-small">
									<i class="fas fa-angle-double-down" />
								</span>
							</button>
						</p>
						<p class="control">
							<button
								type="button"
								class="button is-small"
								title="Move plot down"
								disabled={props.index() === props.total() - 1}
								onClick={(e) => {
									e.preventDefault();
									// Because the ranking algorithm looks backwards,
									// we need to jump ahead, further down the list by two instead of one.
									// Otherwise, the plot will be placed in the same position, albeit with a different rank.
									setRank(props.index() + 2);
								}}
							>
								<span class="icon is-small">
									<i class="fas fa-chevron-down" />
								</span>
							</button>
						</p>
						<p class="control">
							<button
								type="button"
								class="button is-small"
								title="Move plot up"
								disabled={props.index() === 0}
								onClick={(e) => {
									e.preventDefault();
									setRank(props.index() - 1);
								}}
							>
								<span class="icon is-small">
									<i class="fas fa-chevron-up" />
								</span>
							</button>
						</p>
						<p class="control">
							<button
								type="button"
								class="button is-small"
								title="Move plot to top"
								disabled={props.index() === 0}
								onClick={(e) => {
									e.preventDefault();
									setRank(0);
								}}
							>
								<span class="icon is-small">
									<i class="fas fa-angle-double-up" />
								</span>
							</button>
						</p>
					</div>
				</Show>
			</div>

			<div class="level-right">
				<div class="buttons">
					<a
						type="button"
						class="button is-small"
						title="View plot"
						href={`/console/projects/${
							props.plot?.project
						}/perf?${plotQueryString(props.plot)}`}
					>
						<span class="icon is-small">
							<i class="fas fa-external-link-alt" />
						</span>
					</a>
					<button
						type="button"
						class="button is-small"
						title="Plot settings"
						onClick={(e) => {
							e.preventDefault();
							props.handleState(PinnedState.Settings);
						}}
					>
						<span class="icon is-small">
							<i class="fas fa-cog" />
						</span>
					</button>
				</div>
			</div>
		</nav>
	);
};

const PinnedSetting = (props: {
	apiUrl: string;
	params: Params;
	user: JsonAuthUser;
	project: Resource<JsonProject>;
	isAllowed: Resource<boolean>;
	plot: JsonPlot;
	refresh: () => JsonPlot[] | Promise<JsonPlot[]> | null | undefined;
	handleState: (state: PinnedState) => void;
}) => {
	const path = createMemo(
		() => `/v0/projects/${props.plot?.project}/plots/${props.plot?.uuid}`,
	);

	return (
		<>
			<button
				type="button"
				class="button is-small is-fullwidth"
				onClick={(e) => {
					e.preventDefault();
					props.handleState(PinnedState.Front);
				}}
			>
				<span class="icon is-small">
					<i class="fas fa-arrow-left" />
				</span>
				<span>Back to Plot</span>
			</button>
			<br />
			<DeckCard
				apiUrl={props.apiUrl}
				params={props.params}
				user={props.user}
				path={path}
				card={{
					kind: Card.FIELD,
					label: "Title",
					key: "title",
					display: Display.RAW,
					is_allowed: (_apiUrl, _params) => props.isAllowed() === true,
					field: {
						kind: FieldKind.INPUT,
						label: "Title",
						key: "title",
						value: props.plot?.title ?? "",
						valid: null,
						validate: true,
						nullable: true,
						config: plotFields(props.project()).title,
					},
				}}
				data={() => props.plot}
				handleRefresh={props.refresh}
				handleLoopback={props.refresh}
			/>
			<br />
			<Show when={props.isAllowed()}>
				<DeleteButton
					apiUrl={props.apiUrl}
					user={props.user}
					path={path}
					data={() => props.plot}
					subtitle="This plot will no longer appear in your dashboard."
					redirect={(pathname, _data) => pathname}
				/>
			</Show>
		</>
	);
};

export default Pinned;
