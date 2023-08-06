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
import type { Resource } from "../../../config/console";
import consoleConfig, { Operation } from "../../../config/console";
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

const TablePanel = (props: Props) => {
	const bencherConsole = useConsole();
	const [searchParams, setSearchParams] = useSearchParams();
	// const navigate = useNavigate();

	const config = createMemo(
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

	const [refresh, setRefresh] = createSignal(0);
	// const handleRefresh = () => {
	// 	setRefresh(refresh() + 1);
	// };
	const fetcher = createMemo(() => {
		return {
			refresh: refresh(),
			pagination_query: pagination_query(),
			token: bencherConsole?.()?.user()?.token,
		};
	});

	// const [state, setState] = createSignal(TableState.LOADING);
	const getLs = async (fetcher) => {
		const EMPTY_ARRAY = [];
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
		// return await axios(get_options(url, fetcher.token))
		// 	.then((resp) => {
		// 		const data = resp?.data;
		// 		setState(
		// 			data.length === 0
		// 				? page() === 1
		// 					? TableState.EMPTY
		// 					: TableState.END
		// 				: TableState.OK,
		// 		);
		// 		return data;
		// 	})
		// 	.catch((error) => {
		// 		setState(TableState.ERR);
		// 		console.error(error);
		// 		return EMPTY_ARRAY;
		// 	});
		return EMPTY_ARRAY;
	};
	const [tableData] = createResource(fetcher, getLs);

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
			{/* <TableHeader
				config={props.config?.header}
				path_params={props.path_params}
				refresh={refresh}
				handleRefresh={handleRefresh}
			/>
			<Table
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
