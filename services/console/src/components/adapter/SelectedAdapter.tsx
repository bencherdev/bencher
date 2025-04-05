import { Show } from "solid-js";
import { adapter, adapterName, removeAdapter } from "./adapter";

const SelectedAdapter = () => {
	return (
		<Show when={adapter()}>
			<div class="notification mb-4">
				<button
					class="delete"
					onMouseDown={(e) => {
						e.preventDefault();
						removeAdapter();
					}}
				/>
				{adapterName(adapter())}
			</div>
		</Show>
	);
};

export default SelectedAdapter;
