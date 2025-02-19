const IconTitle = (props: {
	icon: string;
	title: string | number | undefined;
}) => (
	<span class="icon-text">
		<span class="icon">
			<i class={props.icon} />
		</span>
		<span>&nbsp;{props.title}</span>
	</span>
);

export default IconTitle;
