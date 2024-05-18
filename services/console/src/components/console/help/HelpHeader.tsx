import { decodePath } from "../../../util/url";

const HelpHeader = () => {
	return (
		<nav class="level">
			<div class="level-left">
				<a
					class="button"
					title="Back"
					href={decodePath("/console")}
				>
					<span class="icon">
						<i class="fas fa-chevron-left" aria-hidden="true" />
					</span>
					<span>Back</span>
				</a>
			</div>
			<div class="level-left">
				<div class="level-item">
					<h3 class="title is-3">Help</h3>
				</div>
			</div>

			<div class="level-right" />
		</nav>
	);
};

export default HelpHeader;
