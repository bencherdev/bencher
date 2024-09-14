import { type Accessor, Match, type Resource, Switch } from "solid-js";
import FieldCard from "./FieldCard";
import type { JsonAuthUser } from "../../../../../types/bencher";
import { Card } from "../../../../../config/types";
import { fmtNestedValue } from "../../../../../util/resource";
import type CardConfig from "./CardConfig";
import type { Params } from "astro";
import ReportCard from "./ReportCard";

export interface Props {
	isConsole: boolean;
	apiUrl: string;
	params: Params;
	user: JsonAuthUser;
	path: Accessor<string>;
	card: CardConfig;
	data: Resource<object>;
	handleRefresh: () => void;
	handleLoopback: (pathname: null | string) => void;
}

const DeckCard = (props: Props) => {
	return (
		<div style="margin-bottom: 0.5rem;">
			<Switch>
				<Match when={props.card?.kind === Card.FIELD}>
					<FieldCard
						isConsole={props.isConsole}
						apiUrl={props.apiUrl}
						params={props.params}
						user={props.user}
						path={props.path}
						card={props.card}
						value={
							props.card?.key ? props.data()?.[props.card?.key] : props.data()
						}
						handleRefresh={props.handleRefresh}
						handleLoopback={props.handleLoopback}
					/>
				</Match>
				<Match when={props.card?.kind === Card.NESTED_FIELD}>
					<FieldCard
						isConsole={props.isConsole}
						apiUrl={props.apiUrl}
						params={props.params}
						user={props.user}
						path={props.path}
						card={props.card}
						value={fmtNestedValue(props.data(), props.card?.keys)}
						handleRefresh={props.handleRefresh}
						handleLoopback={props.handleLoopback}
					/>
				</Match>
				<Match when={props.card?.kind === Card.REPORT}>
					<ReportCard
						isConsole={props.isConsole}
						params={props.params}
						value={props.data}
					/>
				</Match>
			</Switch>
		</div>
	);
};

export default DeckCard;
