import { useNavigate } from "../../../util/url";

const HelpHeader = () => {
	const navigate = useNavigate();

	return (
		<nav class="level">
			<div class="level-left">
				<button
					class="button is-outlined"
					type="button"
					onClick={(e) => {
						e.preventDefault();
						navigate("/console");
					}}
				>
					<span class="icon">
						<i class="fas fa-chevron-left" aria-hidden="true" />
					</span>
					<span>Back</span>
				</button>
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
