import type { Params } from "astro";
import { type Accessor, Match, type Resource, Switch } from "solid-js";
import { Card } from "../../../../../config/types";
import type { JsonAuthUser } from "../../../../../types/bencher";
import { fmtNestedValue } from "../../../../../util/resource";
import type CardConfig from "./CardConfig";
import FieldCard from "./FieldCard";
import ReportCard from "./ReportCard";
import ReportTableCard from "./ReportTableCard";
import ThresholdTableCard from "./ThresholdTableCard";

export interface Props {
	isConsole: boolean;
	apiUrl: string;
	isBencherCloud?: boolean;
	params: Params;
	user: JsonAuthUser;
	path: Accessor<string>;
	card: CardConfig;
	data: Resource<object>;
	width: Accessor<number>;
	handleRefresh: () => void;
	handleLoopback: (pathname: null | string) => void;
}

const DeckCard = (props: Props) => {
	// This only matters for cards that act differently between Bencher Cloud and Bencher Self-Hosted.
	// So for individual cards, we don't require it to be passed in.
	const isBencherCloud = props.isBencherCloud ?? true;

	return (
		<div style="margin-bottom: 0.5rem;">
			<Switch>
				<Match when={props.card?.kind === Card.FIELD}>
					<FieldCard
						isConsole={props.isConsole}
						apiUrl={props.apiUrl}
						isBencherCloud={isBencherCloud}
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
						isBencherCloud={isBencherCloud}
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
						width={props.width}
					/>
				</Match>
				<Match when={props.card?.kind === Card.REPORT_TABLE}>
					<ReportTableCard
						isConsole={props.isConsole}
						apiUrl={props.apiUrl}
						params={props.params}
						user={props.user}
						dimension={props.card?.dimension}
						value={props.data}
					/>
				</Match>
				<Match when={props.card?.kind === Card.THRESHOLD_TABLE}>
					<ThresholdTableCard
						isConsole={props.isConsole}
						apiUrl={props.apiUrl}
						params={props.params}
						user={props.user}
						dimension={props.card?.dimension}
						value={props.data}
					/>
				</Match>
			</Switch>
		</div>
	);
};

export default DeckCard;
