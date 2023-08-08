import { For, Switch, Match, Resource, Accessor } from "solid-js";
import { Params, pathname, useNavigate } from "../../../util/url";
import { fmtNestedValue, fmtValues } from "../../../util/resource";
import { Row } from "../../../config/types";
import type { Slug } from "../../../types/bencher";
import { fmtDateTime } from "../../../config/util";

export enum TableState {
	LOADING = 0,
	EMPTY = 1,
	OK = 2,
	END = 3,
	ERR = 4,
}

export interface Props {
	config: TableConfig;
	state: Accessor<TableState>;
	tableData: Resource<any[]>;
	page: Accessor<number>;
	handlePage: (page: number) => void;
}

export interface TableConfig {
	url: (pathParams: Params) => string;
	name: string;
	add?: AddButtonConfig;
	row: RowConfig;
}

const Table = (props: Props) => {
	return (
		<Switch fallback={<>ERROR: Unknown table state</>}>
			<Match when={props.state() === TableState.LOADING}>
				<></>
			</Match>

			<Match when={props.state() === TableState.EMPTY}>
				<div class="box">
					{props.config?.add ? (
						<AddButton config={props.config?.add} />
					) : (
						<p>🤷</p>
					)}
				</div>
			</Match>

			<Match when={props.state() === TableState.OK}>
				{" "}
				<div class="pricing-table is-horizontal">
					<For each={props.tableData()}>
						{(datum, _i) => (
							<div class="pricing-plan is-primary">
								<div class="plan-header">
									<RowHeader config={props.config?.row} datum={datum} />
								</div>
								<div class="plan-items">
									<For each={props.config?.row?.items}>
										{(item: ItemConfig, _i) => (
											<div class="plan-item">
												<p style="overflow-wrap:anywhere;">
													<Switch fallback="-">
														<Match when={item.kind === Row.TEXT}>
															{item.key && datum[item.key]}
														</Match>
														<Match when={item.kind === Row.BOOL}>
															{item.text}:{" "}
															{item.key && datum[item.key] ? "true" : "false"}
														</Match>
														<Match when={item.kind === Row.SELECT}>
															{item.key &&
																item.value?.options.reduce((field, option) => {
																	if (
																		item.key &&
																		datum[item.key] === option.value
																	) {
																		return option.option;
																	} else {
																		return field;
																	}
																}, datum[item.key])}
														</Match>
														<Match when={item.kind === Row.NESTED_TEXT}>
															{item.keys && fmtNestedValue(datum, item.keys)}
														</Match>
													</Switch>
												</p>
											</div>
										)}
									</For>
								</div>
								<div class="plan-footer">
									<RowButton config={props.config?.row?.button} datum={datum} />
								</div>
							</div>
						)}
					</For>
				</div>
			</Match>

			<Match when={props.state() === TableState.END}>
				<div class="box">
					<BackButton
						name={props.config?.name}
						page={props.page}
						handlePage={props.handlePage}
					/>
				</div>
			</Match>

			<Match when={props.state() === TableState.ERR}>
				<LogoutButton />
			</Match>
		</Switch>
	);
};

export interface AddButtonConfig {
	prefix: Element;
	path: (pathname: string) => string;
	text: string;
}

const AddButton = (props: {
	config: AddButtonConfig;
}) => {
	return (
		<>
			<div class="content has-text-centered">{props.config?.prefix}</div>
			<a
				class="button is-primary is-fullwidth"
				href={props.config?.path?.(pathname())}
			>
				{props.config?.text}
			</a>
		</>
	);
};

export interface RowConfig {
	kind?: Row;
	key: string;
	keys?: string[][];
	items: ItemConfig[];
	button: RowsButtonConfig;
}

export interface ItemConfig {
	kind: Row;
	key?: string;
	keys?: string[];
	text?: string;
	value?: { options: { option: string; value: string }[] };
}

export interface RowsButtonConfig {
	text: string;
	path: (pathname: string, datum: { [slug: string]: Slug }) => string;
}

const RowHeader = (props: {
	config: RowConfig;
	datum: Record<string, any>;
}) => {
	if (props.config?.kind === Row.DATE_TIME && props.config?.key) {
		return fmtDateTime(props.datum[props.config?.key]);
	}
	const header = fmtValues(
		props.datum,
		props.config?.key,
		props.config?.keys,
		" | ",
	);
	return <p style="overflow-wrap:anywhere;">{header}</p>;
};

const RowButton = (props: {
	config: RowsButtonConfig;
	datum: Record<string, any>;
}) => {
	const navigate = useNavigate();

	return (
		<button
			class="button is-fullwidth"
			onClick={(e) => {
				e.preventDefault();
				navigate(props.config?.path?.(pathname(), props.datum));
			}}
		>
			{props.config?.text}
		</button>
	);
};

const BackButton = (props: {
	name: string;
	page: Accessor<number>;
	handlePage: (page: number) => void;
}) => {
	return (
		<button
			class="button is-primary is-fullwidth"
			onClick={(e) => {
				e.preventDefault();
				props.handlePage(props.page() - 1);
			}}
		>
			That's all the {props.name}. Go back.
		</button>
	);
};

const LogoutButton = () => {
	return (
		<>
			<div class="content has-text-centered">Expired session token</div>
			<a class="button is-primary is-fullwidth" href="/auth/logout">
				Log out
			</a>
		</>
	);
};

export default Table;
