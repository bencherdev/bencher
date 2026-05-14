import { Show } from "solid-js";
import { ApiState, apiState } from "../../util/connectivity";

const ReconnectingBanner = () => {
	return (
		<Show when={apiState() !== ApiState.CONNECTED}>
			<section class="section">
				<div class="container">
					<Show
						when={apiState() === ApiState.RECONNECTING}
						fallback={
							<div class="reconnecting-banner is-danger">
								<div class="columns is-vcentered">
									<div class="column">Unable to reach the Bencher API.</div>
									<div class="column is-narrow">
										<button
											class="button"
											type="button"
											onClick={() => window.location.reload()}
										>
											Retry Now
										</button>
									</div>
								</div>
							</div>
						}
					>
						<div class="reconnecting-banner is-warning">
							<span class="reconnecting-dot" />
							Reconnecting to the Bencher API...
						</div>
					</Show>
				</div>
			</section>
		</Show>
	);
};

export default ReconnectingBanner;
