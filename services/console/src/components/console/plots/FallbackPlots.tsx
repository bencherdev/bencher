import FallbackPlot from "./FallbackPlot";

const FallbackPlots = () => {
	return (
		<div class="columns is-multiline is-vcentered">
			<div class="column is-11-tablet is-12-desktop is-6-widescreen">
				<FallbackPlot spaced={true} />
			</div>
			<div class="column is-11-tablet is-12-desktop is-6-widescreen">
				<FallbackPlot spaced={true} />
			</div>
			<div class="column is-11-tablet is-12-desktop is-6-widescreen">
				<FallbackPlot spaced={true} />
			</div>
			<div class="column is-11-tablet is-12-desktop is-6-widescreen">
				<FallbackPlot spaced={true} />
			</div>
		</div>
	);
};

export default FallbackPlots;
