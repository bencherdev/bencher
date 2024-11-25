const FallbackPlot = (props: { spaced?: boolean }) => {
	return (
		<div>
			<div class="columns">
				<div class={`column${props.spaced ? " is-11" : ""}`}>
					<div class="panel">
						<nav class="panel-heading columns is-vcentered">
							<div class="column">&nbsp;</div>
						</nav>
						<div class="panel-block">
							<progress
								class="progress is-primary"
								style="margin-top: 8rem; margin-bottom: 16rem;"
								max="100"
							/>
						</div>
					</div>
				</div>
			</div>
		</div>
	);
};

export default FallbackPlot;
