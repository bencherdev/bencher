// import axios from "axios";
// import { createMemo, createResource, createSignal, For } from "solid-js";
// import { get_options } from "../../site/util";
// import Pagination, { PaginationSize } from "../../site/Pagination";

const Radio = (props) => {
	// const per_page = 8;
	// const [page, setPage] = createSignal(1);

	// const radioFetcher = createMemo(() => {
	// 	return {
	// 		url: props.config?.url(props.path_params, per_page, page()),
	// 		token: props.user?.token,
	// 	};
	// });

	// const getRadio = async (fetcher) => {
	// 	return await axios(get_options(fetcher.url, fetcher.token))
	// 		.then((resp) => resp?.data)
	// 		.catch((error) => {
	// 			console.error(error);
	// 			return [];
	// 		});
	// };

	// const [data] = createResource(radioFetcher, getRadio);

	return (
		<>
			{/* <nav class="level is-mobile">
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
			</div> */}
		</>
	);
};

// const BackButton = (props) => {
// 	return (
// 		<button
// 			class="button is-primary is-small is-fullwidth"
// 			onClick={(e) => {
// 				e.preventDefault();
// 				props.handlePage(props.page - 1);
// 			}}
// 		>
// 			That's all the {props.name}. Go back.
// 		</button>
// 	);
// };

export default Radio;
