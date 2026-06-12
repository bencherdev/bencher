import type { Params } from "astro";
import { createSignal } from "solid-js";
import { KEY_ICON } from "../../../config/project/keys";
import { BencherResource } from "../../../config/types";
import { createdUuidPath } from "../../../config/util";
import { pathname } from "../../../util/url";
import type { JsonKeyCreated } from "./KeyAddPanel";

interface Props {
	params: Params;
	resource: BencherResource;
	data: JsonKeyCreated;
}

const KEY_BODY_LEN = 30;

// Mask the random body but keep the `bencher_*_` prefix visible,
// e.g. `bencher_user_******************************`.
const sanitizedKey = (key: string) =>
	`${key.slice(0, key.lastIndexOf("_") + 1)}${"*".repeat(KEY_BODY_LEN)}`;

const keyLabel = (resource: BencherResource) =>
	resource === BencherResource.USER_KEYS ? "API Key" : "Project Key";

const KeyCreated = (props: Props) => {
	const [copied, setCopied] = createSignal(false);
	const [revealed, setRevealed] = createSignal(false);

	const copyKey = async () => {
		try {
			await navigator.clipboard.writeText(props.data.key);
			setCopied(true);
			setTimeout(() => setCopied(false), 2000);
		} catch (e) {
			console.error(e);
			const input = document.getElementById(
				"created-key",
			) as HTMLInputElement | null;
			input?.select();
		}
	};

	const viewPath = () => createdUuidPath(pathname(), props.data);

	return (
		<div class="columns">
			<div class="column">
				<div class="box">
					<h3 class="title is-4">
						<span class="icon mr-2">
							<i class={KEY_ICON} />
						</span>
						<span>{keyLabel(props.resource)} Created</span>
					</h3>

					<article class="message is-warning">
						<div class="message-body">
							Save this key! It will only be shown once.
						</div>
					</article>

					<div class="field">
						<label class="label" for="created-key">
							{keyLabel(props.resource)}
						</label>
						<div class="field has-addons">
							<div class="control is-expanded">
								<input
									id="created-key"
									class="input is-family-monospace"
									type="text"
									value={
										revealed() ? props.data.key : sanitizedKey(props.data.key)
									}
									readonly
								/>
							</div>
							<div class="control">
								<button
									class="button"
									type="button"
									onClick={() => setRevealed(!revealed())}
								>
									<span class="icon">
										<i class={revealed() ? "fas fa-eye-slash" : "fas fa-eye"} />
									</span>
								</button>
							</div>
							<div class="control">
								<button
									class="button is-primary"
									type="button"
									onClick={copyKey}
								>
									<span class="icon">
										<i class={copied() ? "fas fa-check" : "fas fa-copy"} />
									</span>
									<span>{copied() ? "Copied" : "Copy"}</span>
								</button>
							</div>
						</div>
					</div>

					<br />

					<div class="field">
						<p class="control">
							<a class="button is-fullwidth" href={viewPath()}>
								Continue
							</a>
						</p>
					</div>
				</div>
			</div>
		</div>
	);
};

export default KeyCreated;
