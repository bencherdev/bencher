export const ADAPTER_ICON = "fas fa-plug";

const DimensionLabel = (props: {
	icon: string;
	name: string;
	iconClass?: string;
}) => {
	return (
		<div>
			<span class="icon-text">
				<span class={`icon${props.iconClass ?? ""}`}>
					<i class={props.icon} />
				</span>
				<small style="word-break: break-word;">{props.name}</small>
			</span>
		</div>
	);
};

export default DimensionLabel;
