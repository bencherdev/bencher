import Icon from "@bencherdev/ui/Icon";
import Tooltip from "@bencherdev/ui/Tooltip";

/** A line's own way into Explore: its plot there in one click. */
const RowExplore = (props: { href: string; name: string }) => (
	<Tooltip tip="Open in Explore">
		<a class="lr-go" href={props.href}>
			<Icon name="explore" size="sm" />
			<span class="sr-only">Open in Explore: {props.name}</span>
		</a>
	</Tooltip>
);

export default RowExplore;
