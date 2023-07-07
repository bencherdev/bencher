import axios from "axios";
import {
	createSignal,
	createResource,
	createMemo,
	createEffect,
} from "solid-js";
import Table, { TableState } from "./Table";

import TableHeader from "./TableHeader";
import {
	get_options,
	NOTIFY_KIND_PARAM,
	NOTIFY_TEXT_PARAM,
	validate_jwt,
	PLAN_PARAM,
	validate_u32,
} from "../../../site/util";
import { useNavigate, useSearchParams } from "solid-app-router";
import { forward_path } from "../../../site/Forward";
import TableFooter from "./TableFooter";

// const SORT_PARAM = "sort";
// const DIRECTION_PARAM = "direction";
// const PER_PAGE_PARAM = "per_page";
const PAGE_PARAM = "page";

const TablePanel = (props) => {
	const [searchParams, setSearchParams] = useSearchParams();
	const navigate = useNavigate();

	if (!validate_u32(searchParams[PAGE_PARAM])) {
		setSearchParams({ [PAGE_PARAM]: 1 });
	}
	const page = createMemo(() => searchParams[PAGE_PARAM]);

	const pagination_query = createMemo(() => {
		return {
			page: page(),
		};
	});

	const [refresh, setRefresh] = createSignal(0);
	const handleRefresh = () => {
		setRefresh(refresh() + 1);
	};
	const fetcher = createMemo(() => {
		return {
			refresh: refresh(),
			pagination_query: pagination_query(),
			token: props.user?.token,
		};
	});

	const [state, setState] = createSignal(TableState.LOADING);
	const getLs = async (fetcher) => {
		const EMPTY_ARRAY = [];
		if (!validate_jwt(fetcher.token)) {
			return EMPTY_ARRAY;
		}
		const search_params = new URLSearchParams();
		for (const [key, value] of Object.entries(fetcher.pagination_query)) {
			if (value) {
				search_params.set(key, value);
			}
		}
		const url = `${props.config?.table?.url(
			props.path_params(),
		)}?${search_params.toString()}`;
		return await axios(get_options(url, fetcher.token))
			.then((resp) => {
				const data = resp?.data;
				setState(data.length === 0 ? TableState.EMPTY : TableState.OK);
				return data;
			})
			.catch((error) => {
				setState(TableState.ERR);
				console.error(error);
				return EMPTY_ARRAY;
			});
	};
	const [table_data] = createResource(fetcher, getLs);

	const handlePage = (page: number) => {
		if (validate_u32(page.toString())) {
			setSearchParams({ [PAGE_PARAM]: page });
		}
	};

	const redirect = createMemo(() => props.config.redirect?.(table_data()));

	createEffect(() => {
		if (redirect()) {
			navigate(
				forward_path(
					redirect(),
					[NOTIFY_KIND_PARAM, NOTIFY_TEXT_PARAM, PLAN_PARAM],
					[],
				),
			);
		}
	});

	return (
		<>
			<TableHeader
				config={props.config?.header}
				path_params={props.path_params}
				refresh={refresh}
				handleRefresh={handleRefresh}
			/>
			<Table
				config={props.config?.table}
				table_data={table_data}
				state={state}
			/>
			<TableFooter page={page} handlePage={handlePage} />
		</>
	);
};

export default TablePanel;
