import type { Params } from "astro";
import type { FieldValue, FieldValueHandler } from "../Field";
import { For, createMemo, createResource, createSignal } from "solid-js";
import Pagination, { PaginationSize } from "../../site/Pagination";
import { httpGet } from "../../../util/http";
import { authUser } from "../../../util/auth";

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
	url: (params: undefined | Params, per_page: number, page: number) => string;
	help?: string;
	validate: (value: string) => boolean;
}

const Radio = (props: Props) => {
	const params = createMemo(() => props.params);
	const per_page = 8;
	const [page, setPage] = createSignal(1);

	const fetcher = createMemo(() => {
		return {
			url: props.config?.url(params(), per_page, page()),
			token: authUser()?.token,
		};
	});
	const getRadio = async (fetcher: {
		url: string;
		token: undefined | string;
	}) => {
		return await httpGet(props.apiUrl, fetcher.url, fetcher.token)
			.then((resp) => resp?.data)
			.catch((error) => {
				console.error(error);
				return [];
			});
	};
	const [data] = createResource(fetcher, getRadio);

	return (
		<>
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
															name={data()?.name}
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
							{data()?.length === 0 && page() !== 1 && (
								<BackButton
									name={props.config?.name}
									page={page()}
									handlePage={setPage}
								/>
							)}
						</div>
					</div>
				</div>
			</nav>
			<div class="columns">
				<div class="column is-half">
					<Pagination
						size={PaginationSize.SMALL}
						data_len={data()?.length}
						per_page={per_page}
						page={page()}
						handlePage={setPage}
					/>
				</div>
			</div>
		</>
	);
};

const BackButton = (props: {
	name: string;
	page: number;
	handlePage: (page: number) => void;
}) => {
	return (
		<button
			class="button is-primary is-small is-fullwidth"
			onClick={(e) => {
				e.preventDefault();
				props.handlePage(props.page - 1);
			}}
		>
			That's all the {props.name}. Go back.
		</button>
	);
};

export default Radio;
