import * as Sentry from "@sentry/astro";
import type { Params } from "astro";
import {
	For,
	Match,
	type Resource,
	Show,
	Switch,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import { ThresholdDimension } from "../../../../../config/types";
import { resourcePath } from "../../../../../config/util";
import type {
	JsonAuthUser,
	JsonBranch,
	JsonTestbed,
	JsonThreshold,
} from "../../../../../types/bencher";
import { X_TOTAL_COUNT, httpGet } from "../../../../../util/http";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import {
	type InitValid,
	init_valid,
	validJwt,
} from "../../../../../util/valid";
import Pagination, { PaginationSize } from "../../../../site/Pagination";
import { DEFAULT_PAGE, REPORTS_PER_PAGE } from "../../../perf/PerfPanel";
import { TableState } from "../../../table/Table";
import { PAGE_PARAM, PER_PAGE_PARAM } from "../../../table/TablePanel";
import ThresholdRow from "../../../table/rows/ThresholdRow";

export interface Props {
	isConsole?: boolean;
	apiUrl: string;
	params: Params;
	user: JsonAuthUser;
	dimension: ThresholdDimension;
	value: Resource<JsonBranch | JsonTestbed>;
}

const ThresholdTableCard = (props: Props) => {
	const [bencher_valid] = createResource(init_valid);

	const per_page = () => REPORTS_PER_PAGE;
	const [page, setPage] = createSignal(DEFAULT_PAGE);
	const pagination = createMemo(() => {
		return {
			per_page: per_page(),
			page: page(),
		};
	});

	const [state, setState] = createSignal(TableState.LOADING);
	const [totalCount, setTotalCount] = createSignal(0);

	const fetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			value: props.value(),
			pagination: pagination(),
			token: props.user?.token,
		};
	});
	async function getThresholds<T>(fetcher: {
		bencher_valid: InitValid;
		value: JsonBranch | JsonTestbed;
		pagination: { per_page: number; page: number };
		token: string;
	}) {
		const EMPTY_ARRAY: T[] = [];
		if (!fetcher.bencher_valid) {
			return EMPTY_ARRAY;
		}

		if (
			(props.isConsole && !validJwt(fetcher.token)) ||
			!props.params?.project ||
			props.params?.project === "undefined"
		) {
			return EMPTY_ARRAY;
		}

		const searchParams = new URLSearchParams();
		searchParams.set(PER_PAGE_PARAM, fetcher.pagination?.per_page.toString());
		searchParams.set(PAGE_PARAM, fetcher.pagination?.page.toString());
		if (fetcher.value?.archived) {
			searchParams.set("archived", "true");
		}
		switch (props.dimension) {
			case ThresholdDimension.BRANCH:
				searchParams.set("branch", fetcher.value?.uuid);
				break;
			case ThresholdDimension.TESTBED:
				searchParams.set("testbed", fetcher.value?.uuid);
				break;
			case ThresholdDimension.MEASURE:
				searchParams.set("measure", fetcher.value?.uuid);
				break;
		}

		const path = `/v0/projects/${props.params?.project}/thresholds?${searchParams.toString()}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				setState(resp?.data.length === 0 ? TableState.EMPTY : TableState.OK);
				setTotalCount(resp?.headers?.[X_TOTAL_COUNT]);
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				return EMPTY_ARRAY;
			});
	}
	const [thresholds] = createResource<JsonThreshold[]>(fetcher, getThresholds);
	const thresholdDataLength = createMemo(() => thresholds()?.length);

	return (
		<div class="box" style="margin-top: 2rem">
			<h2 class="title is-4">Thresholds</h2>
			<Switch>
				<Match when={state() === TableState.LOADING}>
					<For each={Array(per_page())}>
						{() => (
							<div class="box" style="margin-bottom: 1rem;">
								&nbsp;
								<br />
								&nbsp;
								<br />
								&nbsp;
								<br />
								&nbsp;
							</div>
						)}
					</For>
				</Match>
				<Match when={state() === TableState.EMPTY}>
					<div class="box" style="margin-bottom: 1rem;">
						<p>🐰 No thresholds found</p>
						<Show when={props.isConsole}>
							<br />
							<a
								class="button"
								href={`${resourcePath(props.isConsole)}/${
									props.params?.project
								}/thresholds/add?${BACK_PARAM}=${encodePath()}`}
							>
								Create a Threshold
							</a>
						</Show>
					</div>
				</Match>
				<Match when={state() === TableState.OK}>
					<For each={thresholds()}>
						{(threshold) => (
							<a
								class="box"
								style="margin-bottom: 1rem;"
								href={`${resourcePath(props.isConsole)}/${
									props.params?.project
								}/thresholds/${threshold?.uuid}?${BACK_PARAM}=${encodePath()}`}
							>
								<ThresholdRow threshold={threshold} />
							</a>
						)}
					</For>
					<section class="section">
						<div class="container">
							<Pagination
								size={PaginationSize.SMALL}
								data_len={thresholdDataLength}
								per_page={per_page}
								page={page}
								total_count={totalCount}
								handlePage={setPage}
							/>
						</div>
					</section>
				</Match>
				<Match when={state() === TableState.ERR}>
					<div class="box" style="margin-bottom: 1rem;">
						<p>🐰 Eek! Failed to load thresholds.</p>
					</div>
				</Match>
			</Switch>
		</div>
	);
};

export default ThresholdTableCard;
