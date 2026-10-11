import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import Segmented from "@bencherdev/ui/Segmented";
import { useQueryClient } from "@tanstack/solid-query";
import { Match, Show, Switch, createSignal } from "solid-js";
import type { JsonProjectKeyCreated } from "../../types/bencher";
import ContentSkeleton from "../ContentSkeleton";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import { startQueries } from "./queries";
import { SAMPLE, exportLine, runCommand } from "./setup";

// The card asks again while it shows and the tab is in view, and goes once a report lands.
const POLL = 5_000;
const KEY_NAME = "bencher run";
// Ninety days, as Settings makes a key unless told otherwise.
const KEY_TTL = 90 * 24 * 60 * 60;

/** A version 1 project nothing has reported to yet: how to send the first report. */
const Setup = () => {
	const { api, slug, host } = useProject();
	const client = useQueryClient();
	const bootstrap = useQueryResult(() => consoleProjectQuery(api, slug()));
	useQueryResult(() => ({
		...startQueries(api, slug()).reports,
		refetchInterval: POLL,
	}));
	// Only a Maintainer may make a key.
	const manage = () => bootstrap().data?.permissions.manage === true;
	// The secret lives here only, never in the cache: it shows once.
	const [created, setCreated] = createSignal<JsonProjectKeyCreated>();
	const [creating, setCreating] = createSignal(false);
	const [failed, setFailed] = createSignal(false);
	const [copied, setCopied] = createSignal<string>();
	const create = async () => {
		setCreating(true);
		setFailed(false);
		try {
			const { data } = await api.send<JsonProjectKeyCreated>(
				"POST",
				`/v0/projects/${encodeURIComponent(slug())}/keys`,
				{ name: KEY_NAME, ttl: KEY_TTL },
			);
			// Settings' keys list asks again when it next shows.
			void client.invalidateQueries({ queryKey: ["console", "keys", slug()] });
			setCreated(data);
		} catch {
			setFailed(true);
		} finally {
			setCreating(false);
		}
	};
	const Copy = (props: { what: string; text: string }) => (
		<Button
			size="sm"
			aria-label={`${copied() === props.what ? "Copied" : "Copy"} ${props.what}`}
			onClick={() =>
				navigator.clipboard
					.writeText(props.text)
					.then(() => setCopied(props.what))
					.catch(() => {})
			}
		>
			{copied() === props.what ? "Copied" : "Copy"}
		</Button>
	);
	const command = () => runCommand(slug(), host);
	return (
		// The reader's permissions decide the first step, so the card waits for them.
		<Show when={bootstrap().data} fallback={<ContentSkeleton />}>
			{(project) => (
				<main class="page">
					<section class="pu-setup" aria-labelledby="pu-setup">
						<div class="pu-intro">
							<div class="eyebrow">
								{project().project.name} · BMF v1 · no reports yet
							</div>
							<Heading level={1} size="xl" id="pu-setup">
								Send the first report
							</Heading>
							<p class="lede">
								Runs create everything here: branches, testbeds, benchmarks,
								variants, measures, and thresholds all come from{" "}
								<code>bencher run</code>. Nothing is made by hand.
							</p>
							<Show when={!manage()}>
								<p class="ro-note">
									<Icon name="read-only" />
									Read only. Ask a project Maintainer for access.
								</p>
							</Show>
						</div>
						<div class="pu-step">
							<h2 class="seclabel">1 · A key for this project</h2>
							<Switch>
								<Match when={created()}>
									{(key) => (
										<>
											<div
												class="secret"
												role="status"
												aria-label="Your new project key"
											>
												<span>{key().key}</span>
												<Copy what="the key" text={key().key} />
											</div>
											<p class="fwarn">
												<Icon name="warning" />
												Shown once. Bencher keeps only a hash of it, so copy it
												now.
											</p>
											<pre class="code">
												{exportLine(key().key)}
												<Copy
													what="the export line"
													text={exportLine(key().key)}
												/>
											</pre>
										</>
									)}
								</Match>
								<Match when={manage()}>
									<div class="pu-row">
										<Button
											variant="primary"
											disabled={creating()}
											onClick={create}
										>
											Create key
										</Button>
										<span class="fhelp">
											A project key for <code>bencher run</code>, shown once.
											Settings, Keys lists it and revokes it.
										</span>
									</div>
									<Show when={failed()}>
										<p class="ferror" role="alert">
											Bencher did not make the key. Try again.
										</p>
									</Show>
								</Match>
								<Match when={!manage()}>
									<p class="fhelp">
										A Maintainer creates the key. Export it before you run:
									</p>
									<pre class="code">
										{exportLine("")}
										<span class="muted">the key a Maintainer gives you</span>
									</pre>
								</Match>
							</Switch>
						</div>
						<div class="pu-step">
							<h2 class="seclabel">2 · A results file in BMF v1</h2>
							<div class="pu-row">
								<span class="flab" id="pu-adapter">
									Adapter
								</span>
								<Segmented
									name="setup-adapter"
									aria-labelledby="pu-adapter"
									options={[{ value: "json", label: "json · BMF v1" }]}
									value="json"
									onChange={() => {}}
								/>
								<span class="fhelp">
									Version 1 projects read BMF v1 JSON; harness adapters join it
									once version 1 accepts them.
								</span>
							</div>
							<pre class="code">
								{SAMPLE}
								<Copy what="the sample" text={SAMPLE} />
							</pre>
							<p class="fhelp">
								Save it as <code>results.json</code>, or have your harness write
								the same shape: each benchmark lists its variants, each with its
								parameters and a value per measure.
							</p>
						</div>
						<div class="pu-step">
							<h2 class="seclabel">3 · Run it</h2>
							<pre class="code">
								{command()}
								<Copy what="the command" text={command()} />
							</pre>
							<p class="fhelp">
								The five threshold flags declare a first threshold: a t-test
								that alerts when the latency value rises past its 99% boundary.
								Later runs declare more the same way.
							</p>
							<p class="pu-docs">
								<span class="muted">Docs:</span>
								<a href="/docs/reference/bencher-metric-format/">BMF format</a>
								<a href="/docs/explanation/bencher-run/">bencher run</a>
								<a href="/docs/explanation/thresholds/">Thresholds</a>
								<a href="/docs/how-to/track-benchmarks/">
									Track benchmarks in CI
								</a>
							</p>
						</div>
						<p class="pu-wait" role="status">
							<span class="pu-dot" aria-hidden="true" />
							Waiting for the first report. When it lands, this card gives way
							to Explore, with that report first among its starting points.
						</p>
					</section>
				</main>
			)}
		</Show>
	);
};

export default Setup;
