import * as Sentry from "@sentry/astro";
import type { Params } from "astro";
import {
	For,
	Match,
	type Resource,
	Switch,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import { ReportDimension } from "../../../../../config/types";
import { resourcePath } from "../../../../../config/util";
import type {
	JsonAuthUser,
	JsonBranch,
	JsonReport,
	JsonTestbed,
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
import { ReportRowFields } from "../../../perf/plot/tab/ReportsTab";
import { TableState } from "../../../table/Table";
import { PAGE_PARAM, PER_PAGE_PARAM } from "../../../table/TablePanel";

export interface Props {
	isConsole?: boolean;
	apiUrl: string;
	params: Params;
	user: JsonAuthUser;
	dimension: ReportDimension;
	value: Resource<JsonBranch | JsonTestbed>;
}

const ReportTableCard = (props: Props) => {
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
	async function getReports<T>(fetcher: {
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
			case ReportDimension.BRANCH:
				searchParams.set("branch", fetcher.value?.uuid);
				break;
			case ReportDimension.TESTBED:
				searchParams.set("testbed", fetcher.value?.uuid);
				break;
		}

		const path = `/v0/projects/${props.params?.project}/reports?${searchParams.toString()}`;
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
	const [reports] = createResource<JsonReport[]>(fetcher, getReports);
	const reportDataLength = createMemo(() => reports()?.length);

	return (
		<div class="box" style="margin-top: 1rem">
			<h2 class="title is-4">Recent Reports</h2>
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
						<p>🐰 No reports found</p>
					</div>
				</Match>
				<Match when={state() === TableState.OK}>
					<For each={reports()}>
						{(report) => (
							<a
								class="box"
								style="margin-bottom: 1rem;"
								href={`${resourcePath(props.isConsole)}/${
									props.params?.project
								}/reports/${report?.uuid}?${BACK_PARAM}=${encodePath()}`}
							>
								<ReportRowFields report={report} />
							</a>
						)}
					</For>
					<section class="section">
						<div class="container">
							<Pagination
								size={PaginationSize.SMALL}
								data_len={reportDataLength}
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
						<p>🐰 Eek! Failed to load reports.</p>
					</div>
				</Match>
			</Switch>
		</div>
	);
};

export default ReportTableCard;
