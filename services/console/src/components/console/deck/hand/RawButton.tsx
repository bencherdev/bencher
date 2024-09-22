import { Match, type Resource, Switch, createSignal } from "solid-js";

export interface Props {
	data: Resource<object>;
}

const RawButton = (props: Props) => {
	const [rawClicked, setRawClicked] = createSignal(false);

	return (
		<Switch>
			<Match when={rawClicked() === false}>
				<div class="buttons is-right">
					<button
						class="button is-small"
						type="button"
						onMouseDown={(e) => {
							e.preventDefault();
							setRawClicked(true);
						}}
					>
						<span class="icon">
							<i class="fas fa-x-ray" />
						</span>
						<span>View JSON</span>
					</button>
				</div>
			</Match>
			<Match when={rawClicked() === true}>
				<div class="columns is-reverse-mobile">
					<div class="column">
						<div class="content">
							<pre>
								<code>{JSON.stringify(props.data(), null, 2)}</code>
							</pre>
						</div>
					</div>
					<div class="column is-narrow">
						<div class="buttons is-right">
							<button
								class="button is-small"
								type="button"
								onMouseDown={(e) => {
									e.preventDefault();
									setRawClicked(false);
								}}
							>
								<span class="icon">
									<i class="fas fa-x-ray" />
								</span>
								<span>Hide JSON</span>
							</button>
						</div>
					</div>
				</div>
			</Match>
		</Switch>
	);
};

export default RawButton;
