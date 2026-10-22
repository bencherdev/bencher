import Banner from "@bencherdev/ui/Banner";
import Heading from "@bencherdev/ui/Heading";
import Skeleton from "@bencherdev/ui/Skeleton";
import { useNavigate } from "@solidjs/router";
import { Show, createSignal, onMount } from "solid-js";
import type { JsonGitHubRepositories } from "../../types/bencher";
import { useAccount } from "../account/context";
import { takeGitHubReturn } from "../astro/githubReturn";
import { NEXT_PROJECTS } from "../paths";
import { hold } from "../settings/Connections";
import { failureOf } from "./Settings";

const FAILED = "Bencher did not read your GitHub repositories";

/**
 * Where GitHub sends the reader back with a code, which the page's first
 * script took from the URL: the code becomes the repositories the reader may
 * link, shown in the project's Connections.
 */
const GitHub = () => {
	const { api } = useAccount();
	const navigate = useNavigate();
	const { code, state } = takeGitHubReturn();
	const project = projectOf(state);
	const [failure, setFailure] = createSignal<string>();
	onMount(async () => {
		if (!(code && state)) {
			setFailure(FAILED);
			return;
		}
		try {
			const { data } = await api.send<JsonGitHubRepositories>(
				"POST",
				"/v0/github/exchange",
				{ code, state },
			);
			hold(data);
			navigate(connectionsPath(data.project), {
				replace: true,
				resolve: false,
			});
		} catch (error) {
			setFailure(failureOf(error, FAILED));
		}
	});
	return (
		<main class="page">
			<div class="pagehead">
				<Heading level={1} size="xl">
					GitHub
				</Heading>
			</div>
			<Show when={failure()} fallback={<Skeleton size="card" />}>
				{(message) => (
					<Banner status="error" role="alert" class="load-error">
						<span class="grow">{message()}</span>
						<Show when={project}>
							{(uuid) => (
								<a
									class="ui-button"
									data-variant="secondary"
									data-size="sm"
									href={connectionsPath(uuid())}
								>
									Back to the project
								</a>
							)}
						</Show>
					</Banner>
				)}
			</Show>
		</main>
	);
};

export default GitHub;

const connectionsPath = (project: string) =>
	`${NEXT_PROJECTS}/${encodeURIComponent(project)}/connections`;

/** The project a link state names, read but not verified: only the API trusts it. */
const projectOf = (state: string | undefined) => {
	try {
		const claims = JSON.parse(
			atob(state?.split(".")[1]?.replace(/-/g, "+").replace(/_/g, "/") ?? ""),
		);
		return typeof claims?.project === "string"
			? (claims.project as string)
			: undefined;
	} catch {
		return undefined;
	}
};
