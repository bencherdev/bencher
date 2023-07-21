import { Link, useLocation, useNavigate } from "solid-app-router";
import { For, Switch, Match, createMemo, Show, JSX } from "solid-js";
import { Row } from "../../config/types";
import { concat_values, nested_value } from "../../../site/util";
import { date_time_fmt } from "../../config/util";

export enum TableState {
	LOADING = 0,
	EMPTY = 1,
	OK = 2,
	END = 3,
	ERR = 4,
}

const Table = (props) => {
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);

	return (
		<Switch fallback={<>ERROR: Unknown table state</>}>
			<Match when={props.state() === TableState.LOADING}>
				<></>
			</Match>

			<Match when={props.state() === TableState.EMPTY}>
				<div class="box">
					<AddButton pathname={pathname} add={props.config?.add} />
				</div>
			</Match>

			<Match when={props.state() === TableState.OK}>
				{" "}
				<div class="pricing-table is-horizontal">
					<For each={props.table_data()}>
						{(datum, _i) => (
							<div class="pricing-plan is-primary">
								<div class="plan-header">
									<RowHeader datum={datum} row={props.config?.row} />
								</div>
								<div class="plan-items">
									<For each={props.config?.row?.items}>
										{(item, i) => (
											<div class="plan-item">
												<p style="overflow-wrap:anywhere;">
													<Switch fallback="-">
														<Match when={item.kind === Row.TEXT}>
															{datum[item.key]}
														</Match>
														<Match when={item.kind === Row.BOOL}>
															{item.text}: {datum[item.key] ? "true" : "false"}
														</Match>
														<Match when={item.kind === Row.SELECT}>
															{item.value?.options.reduce((field, option) => {
																if (datum[item.key] === option.value) {
																	return option.option;
																} else {
																	return field;
																}
															}, datum[item.key])}
														</Match>
														<Match when={item.kind === Row.NESTED_TEXT}>
															{nested_value(datum, item.keys)}
														</Match>
													</Switch>
												</p>
											</div>
										)}
									</For>
								</div>
								<div class="plan-footer">
									<RowButton
										config={props.config?.row?.button}
										pathname={pathname}
										datum={datum}
									/>
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

const AddButton = (props) => {
	return (
		<>
			<div class="content has-text-centered">{props.add?.prefix}</div>
			<Link
				class="button is-primary is-fullwidth"
				href={props.add?.path?.(props.pathname?.())}
			>
				{props.add?.text}
			</Link>
		</>
	);
};

const LogoutButton = (_props) => {
	return (
		<>
			<div class="content has-text-centered">Expired session token</div>
			<Link class="button is-primary is-fullwidth" href="/auth/logout">
				Log out
			</Link>
		</>
	);
};

const RowHeader = (props: { datum: any; row: any }) => {
	if (props.row?.kind === Row.DATE_TIME) {
		return date_time_fmt(props.datum[props.row?.key]);
	}
	const header = concat_values(
		props.datum,
		props.row?.key,
		props.row?.keys,
		" | ",
	);
	return <p style="overflow-wrap:anywhere;">{header}</p>;
};

const RowButton = (props) => {
	const navigate = useNavigate();

	return (
		<button
			class="button is-fullwidth"
			onClick={(e) => {
				e.preventDefault();
				navigate(props.config?.path?.(props.pathname?.(), props.datum));
			}}
		>
			{props.config?.text}
		</button>
	);
};

const BackButton = (props) => {
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

export default Table;
