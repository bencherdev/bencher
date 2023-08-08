import { createMemo, Match, Switch } from "solid-js";
import { Card } from "../../config/types";
import FieldCard from "./FieldCard";
import { nested_value } from "../../../site/util";

const DeckCard = (props) => {
	return (
		<Switch fallback={<></>}>
			<Match when={props.card?.kind === Card.FIELD}>
				<FieldCard
					user={props.user}
					card={props.card}
					value={props.data()?.[props.card?.key]}
					path_params={props.path_params}
					url={props.url}
					refresh={props.refresh}
					handleRefresh={props.handleRefresh}
					handleLoopback={props.handleLoopback}
				/>
			</Match>
			<Match when={props.card?.kind === Card.TABLE}>
				<div>Table Card</div>
			</Match>
			<Match when={props.card?.kind === Card.NESTED_FIELD}>
				<FieldCard
					user={props.user}
					card={props.card}
					value={nested_value(props.data(), props.card?.keys)}
					path_params={props.path_params}
					url={props.url}
					refresh={props.refresh}
					handleRefresh={props.handleRefresh}
				/>
			</Match>
		</Switch>
	);
};

export default DeckCard;
