import { Show, createSignal } from "solid-js";

const REFRESH_TIMEOUT = 2110;

export interface Props {
	title: string;
	handleRefresh: () => void;
}

const RefreshButton = (props: Props) => {
	const [refresh, setRefresh] = createSignal(false);

	return (
		<button
			class="button is-fullwidth"
			type="button"
			title={props.title}
			onMouseDown={(e) => {
				e.preventDefault();
				if (refresh()) {
					return;
				}
				setRefresh(true);
				props.handleRefresh();
				setTimeout(() => {
					setRefresh(false);
				}, REFRESH_TIMEOUT);
			}}
		>
			<Show
				when={refresh()}
				fallback={
					<span class="icon">
						<i class="fas fa-sync-alt" />
					</span>
				}
			>
				<span class="icon has-text-success">
					<i class="far fa-check-circle" />
				</span>
			</Show>
			<span>Refresh</span>
		</button>
	);
};

export default RefreshButton;
