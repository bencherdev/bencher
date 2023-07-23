import { useNavigate } from "solid-app-router";

const DeckButton = (props) => {
	const navigate = useNavigate();

	return (
		<div class="columns">
			<div class="column">
				<div class="box">
					<div class="columns">
						<div class="column">
							<button
								class="button is-fullwidth is-primary"
								onClick={(e) => {
									e.preventDefault();
									navigate(props.config.path(props.path_params));
								}}
							>
								Select
							</button>
						</div>
					</div>
				</div>
			</div>
		</div>
	);
};

export default DeckButton;
