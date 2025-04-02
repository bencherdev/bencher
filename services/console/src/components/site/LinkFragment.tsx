const LinkFragment = (props: { fragment: string }) => {
	return (
		<a style="padding-left:0.3em;color:#fdb07e" href={`#${props.fragment}`}>
			<small>
				<i class="fas fa-link" />
			</small>
		</a>
	);
};

export default LinkFragment;
