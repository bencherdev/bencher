import {
	createSignal,
	createResource,
	createMemo,
	createEffect,
} from "solid-js";
import Pagination, { PaginationSize } from "../../site/Pagination";
import { useNavigate, useSearchParams } from "../../../util/url";
import { validJwt, validU32 } from "../../../util/valid";
import consoleConfig from "../../../config/console";
import {
	Operation,
	resourcePlural,
	type Resource,
} from "../../../config/types";
import { httpGet } from "../../../util/http";
import { authUser } from "../../../util/auth";
import bencher_valid_init, { type InitOutput } from "bencher_valid";
import TableHeader, { type TableHeaderConfig } from "./TableHeader";
import Table, { type TableConfig, TableState } from "./Table";
import type { Params } from "astro";
import { NotifyKind, pageNotify } from "../../../util/notify";

const PER_PAGE_PARAM = "per_page";
const PAGE_PARAM = "page";

const DEFAULT_PER_PAGE = 8;
const DEFAULT_PAGE = 1;

interface Props {
	apiUrl: string;
	params: Params;
	resource: Resource;
}

interface TablePanelConfig {
	redirect: (tableData: Record<string, any>[]) => null | string;
	header: TableHeaderConfig;
	table: TableConfig;
}

const TablePanel = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const [searchParams, setSearchParams] = useSearchParams();
	const navigate = useNavigate();

	const config = createMemo<TablePanelConfig>(
		() => consoleConfig[props.resource]?.[Operation.LIST],
	);

	const initParams: Record<string, null | number | boolean> = {};
	if (!validU32(searchParams[PER_PAGE_PARAM])) {
		initParams[PER_PAGE_PARAM] = DEFAULT_PER_PAGE;
	}
	if (!validU32(searchParams[PAGE_PARAM])) {
		initParams[PAGE_PARAM] = DEFAULT_PAGE;
	}
	if (Object.keys(initParams).length !== 0) {
		setSearchParams(initParams);
	}

	const per_page = createMemo(() => Number(searchParams[PER_PAGE_PARAM]));
	const page = createMemo(() => Number(searchParams[PAGE_PARAM]));

	const paginationQuery = createMemo(() => {
		return {
			per_page: per_page(),
			page: page(),
		};
	});

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
		const EMPTY_ARRAY: Record<string, any>[] = [];
		if (!bencher_valid()) {
			return EMPTY_ARRAY;
		}

		if (!validJwt(fetcher.token)) {
			return EMPTY_ARRAY;
		}
		const searchParams = new URLSearchParams();
		for (const [key, value] of Object.entries(fetcher.paginationQuery)) {
			if (value) {
				searchParams.set(key, value.toString());
			}
		}
		const path = `${config()?.table?.url(
			props.params,
		)}?${searchParams.toString()}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
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
				pageNotify(
					NotifyKind.ERROR,
					`Lettuce romaine calm! Failed to fetch ${resourcePlural(
						props.resource,
					)}. Please, try again.`,
				);
				return EMPTY_ARRAY;
			});
	};
	const [tableData, { refetch }] = createResource<Record<string, any>[]>(
		fetcher,
		getData,
	);
	const tableDataLength = createMemo(() => tableData()?.length);

	createEffect(() => {
		const newParams: Record<string, null | number | boolean> = {};
		if (!validU32(searchParams[PER_PAGE_PARAM])) {
			newParams[PER_PAGE_PARAM] = DEFAULT_PER_PAGE;
		}
		if (!validU32(searchParams[PAGE_PARAM])) {
			newParams[PAGE_PARAM] = DEFAULT_PAGE;
		}
		if (Object.keys(newParams).length !== 0) {
			setSearchParams(newParams);
		}
	});

	const handlePage = (page: number) => {
		if (validU32(page)) {
			setSearchParams({ [PAGE_PARAM]: page });
		}
	};

	return (
		<>
			<TableHeader
				apiUrl={props.apiUrl}
				params={props.params}
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
						data_len={tableDataLength}
						per_page={per_page}
						page={page}
						handlePage={handlePage}
					/>
				</div>
			</section>
		</>
	);
};

export default TablePanel;
