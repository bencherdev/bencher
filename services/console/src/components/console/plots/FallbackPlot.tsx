const FallbackPlot = () => {
	return (
		<div>
			<div class="columns">
				<div class="column">
					<div class="panel">
						<nav class="panel-heading">
							<div class="columns is-mobile is-centered is-vcentered is-gapless">
								<div class="column">&nbsp;</div>
							</div>
						</nav>
						<div class="panel-block">
							<progress
								class="progress is-primary"
								style="margin-top: 8rem; margin-bottom: 16rem;"
								max="100"
							/>
						</div>
						<div class="box">
							<nav class="level is-mobile">
								<div class="level-left">
									<div class="level is-mobile">
										<div class="level-item">
											<button
												type="button"
												class="button is-small is-rounded"
												title="Move plot"
												disabled={true}
											>
												&nbsp;
											</button>
										</div>
									</div>
								</div>
								<div class="level-right">
									<div class="buttons">
										<button
											type="button"
											class="button is-small"
											disabled={true}
											title="View plot"
										>
											<span class="icon is-small">
												<i class="fas fa-external-link-alt" />
											</span>
										</button>
										<button
											type="button"
											class="button is-small"
											disabled={true}
											title="Plot settings"
										>
											<span class="icon is-small">
												<i class="fas fa-cog" />
											</span>
										</button>
									</div>
								</div>
							</nav>
						</div>
					</div>
				</div>
			</div>
		</div>
	);
};

export default FallbackPlot;
