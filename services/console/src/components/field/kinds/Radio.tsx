import type { Params } from "astro";
import type { FieldValue, FieldValueHandler } from "../Field";
import { For, createMemo, createResource, createSignal } from "solid-js";
import Pagination, { PaginationSize } from "../../site/Pagination";
import { X_TOTAL_COUNT, httpGet } from "../../../util/http";
import { authUser } from "../../../util/auth";
import * as Sentry from "@sentry/astro";
import Field from "../Field";
import FieldKind from "../kind";
import { toCapitalized } from "../../../config/util";
import { debounce } from "@solid-primitives/scheduled";
import { DEBOUNCE_DELAY } from "../../../util/valid";
import { DEFAULT_PAGE, DEFAULT_PER_PAGE } from "../../console/perf/PerfPanel";

export type RadioValue = string;

export interface Props {
	apiUrl: string;
	value: FieldValue;
	config: RadioConfig;
	params: undefined | Params;
	handleField: FieldValueHandler;
}

export interface RadioConfig {
	name: string;
	icon: string;
	option_key: string;
	value_key: string;
	url: (
		params: undefined | Params,
		per_page: number,
		page: number,
		search: undefined | string,
	) => string;
	help?: string;
	validate: (value: string) => boolean;
}

const Radio = (props: Props) => {
	const params = createMemo(() => props.params);

	const per_page = createMemo(() => DEFAULT_PER_PAGE);
	const [page, setPage] = createSignal(DEFAULT_PAGE);

	const [search, setSearch] = createSignal<undefined | string>();
	const handleSearch = (_key: string, search: string, _valid: boolean) =>
		debounce((search: string) => {
			console.log(search);
			setPage(DEFAULT_PAGE);
			setSearch(search);
			return;
		}, DEBOUNCE_DELAY)(search);

	const [totalCount, setTotalCount] = createSignal(0);
	const fetcher = createMemo(() => {
		return {
			url: props.config?.url(params(), per_page(), page(), search()),
			token: authUser()?.token,
		};
	});
	const getRadio = async (fetcher: {
		url: string;
		token: undefined | string;
	}) => {
		return await httpGet(props.apiUrl, fetcher.url, fetcher.token)
			.then((resp) => {
				setTotalCount(resp?.headers?.[X_TOTAL_COUNT]);
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				return [];
			});
	};
	const [data] = createResource(fetcher, getRadio);
	const dataLength = createMemo(() => data()?.length);

	return (
		<>
			<nav class="level is-mobile">
				<Field
					kind={FieldKind.SEARCH}
					fieldKey="search"
					value={search() ?? ""}
					config={{
						placeholder: `Search ${toCapitalized(props.config?.name ?? "")}`,
					}}
					handleField={handleSearch}
				/>
			</nav>
			<nav class="level is-mobile">
				<div class="level-left">
					<div class="level-item">
						<div class="icon is-small is-left">
							<i class={props.config.icon} />
						</div>
					</div>
					<div class="level-item">
						<div class="control">
							<For each={data()}>
								{(datum) => (
									<>
										<label class="radio">
											<nav class="level is-mobile">
												<div class="level-left">
													<div class="level-item">
														<input
															type="radio"
															name={props.config?.name}
															checked={
																props.value === datum[props.config?.value_key]
															}
															onInput={(_event) =>
																props.handleField(
																	datum[props.config?.value_key],
																)
															}
														/>
													</div>
													<div class="level-item">
														{datum[props.config?.option_key]}
													</div>
												</div>
											</nav>
										</label>
										<br />
									</>
								)}
							</For>
						</div>
					</div>
				</div>
			</nav>
			<div class="columns">
				<div class="column is-half">
					<Pagination
						size={PaginationSize.SMALL}
						data_len={dataLength}
						per_page={per_page}
						page={page}
						total_count={totalCount}
						handlePage={setPage}
					/>
				</div>
			</div>
		</>
	);
};

export default Radio;
