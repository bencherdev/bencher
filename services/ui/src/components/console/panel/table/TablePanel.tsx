import axios from "axios";
import {
	createSignal,
	createResource,
	createMemo,
	createEffect,
} from "solid-js";
import Table from "./Table";

import TableHeader from "./TableHeader";
import {
	get_options,
	NOTIFY_KIND_PARAM,
	NOTIFY_TEXT_PARAM,
	validate_jwt,
} from "../../../site/util";
import { useNavigate } from "solid-app-router";
import Forward, { forward_path } from "../../../site/Forward";
import { PLAN_PARAM } from "../../../auth/AuthForm";

const TablePanel = (props) => {
	const navigate = useNavigate();

	const url = createMemo(() => props.config?.table?.url(props.path_params()));

	const [refresh, setRefresh] = createSignal(0);
	const handleRefresh = () => {
		setRefresh(refresh() + 1);
	};
	const [page, setPage] = createSignal(1);
	const fetcher = createMemo(() => {
		return {
			refresh: refresh(),
			page: page(),
			token: props.user?.token,
		};
	});

	const getLs = async (fetcher) => {
		const EMPTY_ARRAY = [];
		try {
			if (!validate_jwt(fetcher.token)) {
				return EMPTY_ARRAY;
			}

			const resp = await axios(get_options(url(), fetcher.token));
			return resp.data;
		} catch (error) {
			console.error(error);
			return EMPTY_ARRAY;
		}
	};
	const [table_data] = createResource(fetcher, getLs);

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
			<Table config={props.config?.table} table_data={table_data} />
		</>
	);
};

export default TablePanel;
