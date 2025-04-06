import { Show } from "solid-js";
import { adapter, adapterIcon, removeAdapter } from "./adapter";
import type { Adapter } from "../../types/bencher";
import { adapterName } from "./name";

const SelectedAdapter = () => {
	return (
		<Show when={adapter()}>
			<div class="notification mb-4">
				<button
					class="delete"
					type="button"
					onMouseDown={(e) => {
						e.preventDefault();
						removeAdapter();
					}}
				/>
				<div class="columns is-mobile is-vcentered is-gapless">
					<div class="column is-narrow">
						<span class="icon has-text-primary is-large">
							<i class={`${adapterIcon(adapter() as Adapter)} fa-2x`} />
						</span>
					</div>
					<div class="column is-narrow">
						<div>{adapterName(adapter() as Adapter)}</div>
					</div>
				</div>
			</div>
		</Show>
	);
};

export default SelectedAdapter;
