import axios from "axios";
import {
	createSignal,
	createResource,
	createMemo,
	createEffect,
} from "solid-js";
import Pagination, { PaginationSize } from "../../site/Pagination";
import { useSearchParams } from "../../../util/url";
import { useConsole } from "../Console";
import { validJwt } from "../../../util/valid";
import consoleConfig from "../../../config/console";
import { Operation, type Resource } from "../../../config/types";
import { httpGet } from "../../../util/http";
import { authUser } from "../../../util/auth";
import bencher_valid_init from "bencher_valid";
import TableHeader, { TableHeaderConfig } from "./TableHeader";
// import Table, { TableState } from "./Table";

// import TableHeader from "./TableHeader";
// import {
// 	get_options,
// 	NOTIFY_KIND_PARAM,
// 	NOTIFY_TEXT_PARAM,
// 	validate_jwt,
// 	PLAN_PARAM,
// 	validate_u32,
// } from "../../../site/util";
// import { useNavigate, useSearchParams } from "solid-app-router";
// import { forward_path } from "../../../site/Forward";
// import TableFooter from "./TableFooter";
// import Pagination, { PaginationSize } from "../../../site/Pagination";

// const SORT_PARAM = "sort";
// const DIRECTION_PARAM = "direction";
const PER_PAGE_PARAM = "per_page";
const PAGE_PARAM = "page";

const DEFAULT_PER_PAGE = 8;
const DEFAULT_PAGE = 1;

interface Props {
	pathParams: Record<string, string>;
	resource: Resource;
}

interface TableConfig {
	header: TableHeaderConfig;
}

const TablePanel = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const [searchParams, setSearchParams] = useSearchParams();
	// const navigate = useNavigate();

	const config = createMemo<TableConfig>(
		() => consoleConfig[props.resource]?.[Operation.LIST],
	);

	// if (!validate_u32(searchParams[PER_PAGE_PARAM])) {
	// 	setSearchParams({ [PER_PAGE_PARAM]: DEFAULT_PER_PAGE });
	// }
	// if (!validate_u32(searchParams[PAGE_PARAM])) {
	// 	setSearchParams({ [PAGE_PARAM]: DEFAULT_PAGE });
	// }

	const per_page = createMemo(() => Number(searchParams[PER_PAGE_PARAM]));
	const page = createMemo(() => Number(searchParams[PAGE_PARAM]));

	const pagination_query = createMemo(() => {
		return {
			per_page: per_page(),
			page: page(),
		};
	});

	// const [refresh, setRefresh] = createSignal(0);
	// const handleRefresh = () => {
	// 	setRefresh(refresh() + 1);
	// };
	const fetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			pagination_query: pagination_query(),
			token: authUser()?.token,
		};
	});

	// const [state, setState] = createSignal(TableState.LOADING);
	const getData = async (fetcher) => {
		const EMPTY_ARRAY = [];
		console.log("start");
		if (!bencher_valid()) {
			return EMPTY_ARRAY;
		}
		console.log("here");

		if (!validJwt(fetcher.token)) {
			return EMPTY_ARRAY;
		}
		const urlSearchParams = new URLSearchParams();
		for (const [key, value] of Object.entries(fetcher.pagination_query)) {
			if (value) {
				urlSearchParams.set(key, value.toString());
			}
		}
		const url = `${config()?.table?.url(
			props.pathParams,
		)}?${urlSearchParams.toString()}`;
		// return EMPTY_ARRAY;
		return await httpGet(url, fetcher.token)
			.then((resp) => {
				const data = resp?.data;
				console.log(data);
				// setState(
				// 	data.length === 0
				// 		? page() === 1
				// 			? TableState.EMPTY
				// 			: TableState.END
				// 		: TableState.OK,
				// );
				return data;
			})
			.catch((error) => {
				// setState(TableState.ERR);
				console.error(error);
				return EMPTY_ARRAY;
			});
	};
	const [tableData, { refetch }] = createResource(fetcher, getData);

	// createEffect(() => {
	// 	if (!validate_u32(searchParams[PER_PAGE_PARAM])) {
	// 		setSearchParams({ [PER_PAGE_PARAM]: DEFAULT_PER_PAGE });
	// 	}
	// });
	// createEffect(() => {
	// 	if (!validate_u32(searchParams[PAGE_PARAM])) {
	// 		setSearchParams({ [PAGE_PARAM]: DEFAULT_PAGE });
	// 	}
	// });

	const handlePage = (page: number) => {
		// if (validate_u32(page.toString())) {
		// 	setSearchParams({ [PAGE_PARAM]: page });
		// }
	};

	// const redirect = createMemo(() => props.config.redirect?.(tableData()));

	// createEffect(() => {
	// 	if (redirect()) {
	// 		navigate(
	// 			forward_path(
	// 				redirect(),
	// 				[NOTIFY_KIND_PARAM, NOTIFY_TEXT_PARAM, PLAN_PARAM],
	// 				[],
	// 			),
	// 		);
	// 	}
	// });

	return (
		<>
			<TableHeader
				pathParams={props.pathParams}
				config={config()?.header}
				handleRefresh={refetch}
			/>
			{/* <Table
				config={props.config?.table}
				tableData={tableData}
				state={state}
				page={page}
				handlePage={handlePage}
			/> */}
			<section class="section">
				<div class="container">
					<Pagination
						size={PaginationSize.REGULAR}
						data_len={tableData()?.length}
						per_page={per_page()}
						page={page()}
						handlePage={handlePage}
					/>
				</div>
			</section>
		</>
	);
};

export default TablePanel;
