import { Show } from "solid-js";
import { adapterIcon, clearAdapter } from "./adapter";
import { Language } from "../../i18n/ui";
import type { Adapter } from "../../types/bencher";
import { adapterDocsUrl, adapterName } from "./name";
import { adapter } from "./util";

const SelectedAdapterInner = (props: { lang: undefined | string }) => {
	const lang = (props.lang as Language) ?? Language.en;

	return (
		<Show when={adapter()}>
			<div class="notification mb-4">
				<button
					class="delete"
					type="button"
					onMouseDown={(e) => {
						e.preventDefault();
						clearAdapter();
					}}
				/>
				<div class="columns is-mobile is-vcentered is-gapless">
					<div class="column is-narrow">
						<span class="icon has-text-primary is-large">
							<i class={`${adapterIcon(adapter() as Adapter)} fa-2x`} />
						</span>
					</div>
					<div class="column is-narrow">
						<a href={adapterDocsUrl(lang, adapter() as Adapter)}>
							{adapterName(adapter() as Adapter)}
						</a>
					</div>
				</div>
			</div>
		</Show>
	);
};

export default SelectedAdapterInner;
