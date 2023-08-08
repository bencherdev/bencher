import {
	createSignal,
	createResource,
	createMemo,
	createEffect,
} from "solid-js";
import Pagination, { PaginationSize } from "../../site/Pagination";
import { useParams, useSearchParams } from "../../../util/url";
import { validJwt, validU32 } from "../../../util/valid";
import consoleConfig from "../../../config/console";
import { Operation, type Resource } from "../../../config/types";
import { httpGet } from "../../../util/http";
import { authUser } from "../../../util/auth";
import bencher_valid_init, { InitOutput } from "bencher_valid";
import TableHeader, { TableHeaderConfig } from "./TableHeader";
import Table, { TableConfig, TableState } from "./Table";
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
	path: string;
	resource: Resource;
}

interface TablePanelConfig {
	header: TableHeaderConfig;
	table: TableConfig;
}

const TablePanel = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const pathParams = useParams(props.path);
	const [searchParams, setSearchParams] = useSearchParams();
	// const navigate = useNavigate();

	const config = createMemo<TablePanelConfig>(
		() => consoleConfig[props.resource]?.[Operation.LIST],
	);

	if (!validU32(searchParams[PER_PAGE_PARAM])) {
		setSearchParams({ [PER_PAGE_PARAM]: DEFAULT_PER_PAGE });
	}
	if (!validU32(searchParams[PAGE_PARAM])) {
		setSearchParams({ [PAGE_PARAM]: DEFAULT_PAGE });
	}

	const per_page = createMemo(() => Number(searchParams[PER_PAGE_PARAM]));
	const page = createMemo(() => Number(searchParams[PAGE_PARAM]));

	const paginationQuery = createMemo(() => {
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
			paginationQuery: paginationQuery(),
			token: authUser()?.token,
		};
	});

	const [state, setState] = createSignal(TableState.LOADING);
	const getData = async (fetcher: {
		bencher_valid: InitOutput;
		paginationQuery: {
			per_page: number;
			page: number;
		};
		token: string;
	}) => {
		const EMPTY_ARRAY = [];
		if (!bencher_valid()) {
			return EMPTY_ARRAY;
		}

		if (!validJwt(fetcher.token)) {
			return EMPTY_ARRAY;
		}
		const urlSearchParams = new URLSearchParams();
		for (const [key, value] of Object.entries(fetcher.paginationQuery)) {
			if (value) {
				urlSearchParams.set(key, value.toString());
			}
		}
		const url = `${config()?.table?.url(
			pathParams,
		)}?${urlSearchParams.toString()}`;
		return await httpGet(url, fetcher.token)
			.then((resp) => {
				setState(
					resp?.data.length === 0
						? page() === 1
							? TableState.EMPTY
							: TableState.END
						: TableState.OK,
				);
				return resp?.data;
			})
			.catch((error) => {
				setState(TableState.ERR);
				console.error(error);
				return EMPTY_ARRAY;
			});
	};
	const [tableData, { refetch }] = createResource(fetcher, getData);

	createEffect(() => {
		if (!validU32(searchParams[PER_PAGE_PARAM])) {
			setSearchParams({ [PER_PAGE_PARAM]: DEFAULT_PER_PAGE });
		}
		if (!validU32(searchParams[PAGE_PARAM])) {
			setSearchParams({ [PAGE_PARAM]: DEFAULT_PAGE });
		}
	});

	const handlePage = (page: number) => {
		if (validU32(page)) {
			setSearchParams({ [PAGE_PARAM]: page });
		}
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
			<Table
				config={config()?.table}
				state={state}
				tableData={tableData}
				page={page}
				handlePage={handlePage}
			/>
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
