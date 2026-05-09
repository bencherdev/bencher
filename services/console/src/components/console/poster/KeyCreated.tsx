import type { Params } from "astro";
import { createSignal } from "solid-js";
import type { JsonProjectKeyCreated } from "../../../types/bencher";
import { KEY_ICON } from "../../../config/project/keys";
import { createdUuidPath } from "../../../config/util";
import { pathname } from "../../../util/url";

interface Props {
	params: Params;
	data: JsonProjectKeyCreated;
}

const PROJECT_KEY_PREFIX = "bencher_run_";
const maskKey = (key: string) =>
	`${PROJECT_KEY_PREFIX}${"*".repeat(key.length - PROJECT_KEY_PREFIX.length)}`;

const KeyCreated = (props: Props) => {
	const [copied, setCopied] = createSignal(false);
	const [revealed, setRevealed] = createSignal(false);

	const copyKey = async () => {
		await navigator.clipboard.writeText(props.data.key);
		setCopied(true);
		setTimeout(() => setCopied(false), 2000);
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
						<span>Project Key Created</span>
					</h3>

					<article class="message is-warning">
						<div class="message-body">
							Save this key! It will only be shown once.
						</div>
					</article>

					<div class="field">
						<label class="label" for="project-key">
							Project Key
						</label>
						<div class="field has-addons">
							<div class="control is-expanded">
								<input
									id="project-key"
									class="input is-family-monospace"
									type="text"
									value={revealed() ? props.data.key : maskKey(props.data.key)}
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
