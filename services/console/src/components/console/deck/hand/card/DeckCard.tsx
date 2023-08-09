import { Accessor, Match, Resource, Switch } from "solid-js";
import FieldCard from "./FieldCard";
import type { Params } from "../../../../../util/url";
import type { JsonAuthUser } from "../../../../../types/bencher";
import { Card } from "../../../../../config/types";
import { fmtNestedValue } from "../../../../../util/resource";
import type CardConfig from "./CardConfig";

export interface Props {
	pathParams: Params;
	user: JsonAuthUser;
	url: Accessor<string>;
	card: CardConfig;
	data: Resource<Record<string, any>>;
	handleRefresh: () => void;
	handleLoopback: (pathname: null | string) => void;
}

const DeckCard = (props: Props) => {
	return (
		<Switch fallback={<></>}>
			<Match when={props.card?.kind === Card.FIELD}>
				<FieldCard
					pathParams={props.pathParams}
					user={props.user}
					url={props.url}
					card={props.card}
					value={props.card?.key ? props.data()?.[props.card?.key] : null}
					// refresh={props.refresh}
					handleRefresh={props.handleRefresh}
					handleLoopback={props.handleLoopback}
				/>
			</Match>
			<Match when={props.card?.kind === Card.TABLE}>
				<div>Table Card</div>
			</Match>
			<Match when={props.card?.kind === Card.NESTED_FIELD}>
				<FieldCard
					pathParams={props.pathParams}
					user={props.user}
					url={props.url}
					card={props.card}
					value={fmtNestedValue(props.data(), props.card?.keys)}
					// refresh={props.refresh}
					handleRefresh={props.handleRefresh}
					handleLoopback={props.handleLoopback}
				/>
			</Match>
		</Switch>
	);
};

export default DeckCard;
