import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import Selector from "@bencherdev/ui/Selector";
import Skeleton from "@bencherdev/ui/Skeleton";
import TextInput from "@bencherdev/ui/TextInput";
import Truncate from "@bencherdev/ui/Truncate";
import { useNavigate, useParams } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import { For, Show, createMemo, createSignal, useContext } from "solid-js";
import type {
	JsonConsoleUser,
	JsonNewProject,
	JsonProject,
	Visibility,
} from "../../types/bencher";
import { useAccount } from "../account/context";
import { FlushContext } from "../flush";
import { organizationIn } from "../organization/data";
import { privateQuery } from "../organization/visibility";
import { projectPath } from "../paths";
import { consoleUserQuery } from "../queries";
import { useQueryResult } from "../query";
import { nameProblem, slugOf } from "./Organizations";
import { failureOf } from "./Profile";

/** A project for one of the reader's organizations, landing on its root. */
const NewProject = () => {
	const { api, reader } = useAccount();
	const client = useQueryClient();
	const flush = useContext(FlushContext);
	const navigate = useNavigate();
	const params = useParams();
	const key = () => params.organization ?? "";
	const bootstrap = useQueryResult(() =>
		consoleUserQuery(api, reader().user.uuid),
	);
	const plan = useQueryResult(() => privateQuery(api, key()));
	const organization = () => organizationIn(bootstrap().data, key());
	const organizations = createMemo(() =>
		[...(bootstrap().data?.organizations ?? [])].sort((a, b) =>
			a.name.localeCompare(b.name),
		),
	);

	const [name, setName] = createSignal("");
	const [url, setUrl] = createSignal("");
	// Literals, not the generated enum, keep every enum's code out of the page.
	const [chosen, setChosen] = createSignal<`${Visibility}`>("public");
	const mayPrivate = () => plan().data === true;
	const visibility = () => (mayPrivate() ? chosen() : "public");
	const problem = () => nameProblem(name());
	const [creating, setCreating] = createSignal(false);
	const [failed, setFailed] = createSignal<{ error: unknown }>();

	const create = async (into: string) => {
		setCreating(true);
		setFailed(undefined);
		const body: JsonNewProject = { name: name().trim() };
		if (url().trim()) {
			body.url = url().trim();
		}
		if (visibility() === "private") {
			body.visibility = "private" as Visibility;
		}
		let project: JsonProject;
		try {
			project = (
				await api.send<JsonProject>(
					"POST",
					`/v0/organizations/${into}/projects`,
					body,
				)
			).data;
		} catch (error) {
			setFailed({ error });
			setCreating(false);
			return;
		}
		const { uuid, slug, organization: owner, bmf_version } = project;
		client.setQueryData<JsonConsoleUser>(
			consoleUserQuery(api, reader().user.uuid).queryKey,
			(user) =>
				user && {
					...user,
					projects: [
						...user.projects,
						{
							uuid,
							name: project.name,
							slug,
							organization: owner,
							bmf_version,
							active_alerts: 0,
						},
					],
				},
		);
		// Leaving the app first would keep the bootstrap without it.
		await flush();
		navigate(projectPath(slug), { resolve: false });
	};

	const visibilityOption = (option: `${Visibility}`, label: string) => (
		<label class="ui-segment">
			<input
				type="radio"
				name="np-vis"
				value={option}
				checked={visibility() === option}
				disabled={option === "private" && !mayPrivate()}
				onChange={() => setChosen(option)}
			/>
			{label}
		</label>
	);

	return (
		<main class="page">
			<div class="setbody" style={{ width: "100%", "max-width": "760px" }}>
				<div class="sethead">
					<div class="ph-title">
						<div class="crumb-line">
							{organization()?.name ?? key()}
							<span aria-hidden="true">/</span>
						</div>
						<Heading level={1} size="xl">
							New project
						</Heading>
					</div>
				</div>
				<Show
					when={organization()}
					fallback={
						<Show
							when={bootstrap().error ?? bootstrap().data}
							fallback={<Skeleton size="card" />}
						>
							<Banner status="error" role="alert" class="load-error">
								<span class="grow">New project did not load</span>
								<Button size="sm" onClick={() => bootstrap().refetch()}>
									Retry
								</Button>
							</Banner>
						</Show>
					}
				>
					{(into) => (
						<form
							class="setcard"
							aria-labelledby="np-sec"
							onSubmit={(event) => {
								event.preventDefault();
								if (name().trim() && !problem() && !creating()) {
									create(into().uuid);
								}
							}}
						>
							<div class="setcard-head">
								<h2 class="seclabel" id="np-sec">
									Project
								</h2>
							</div>
							<div class="formrows">
								<div class="formrow">
									<div class="fl">
										<label for="np-org">Organization</label>
									</div>
									<div class="field">
										<Selector
											id="np-org"
											onChange={(event) => {
												const next = organizations().find(
													({ uuid }) => uuid === event.currentTarget.value,
												);
												if (next) {
													navigate(`/organizations/${next.slug}/projects/add`, {
														replace: true,
													});
												}
											}}
										>
											<For each={organizations()}>
												{(option) => (
													// Selector sets its value before its options exist.
													<option
														value={option.uuid}
														selected={option.uuid === into().uuid}
													>
														{option.name}
													</option>
												)}
											</For>
										</Selector>
									</div>
								</div>
								<div class="formrow">
									<div class="fl">
										<label for="np-name">Name</label>
									</div>
									<div class="field">
										<TextInput
											id="np-name"
											value={name()}
											onInput={(event) => setName(event.currentTarget.value)}
											autocomplete="off"
											aria-describedby="np-name-e"
											aria-invalid={problem() ? true : undefined}
										/>
										<Show when={problem()}>
											{(message) => (
												<p class="ferror" id="np-name-e">
													{message()}
												</p>
											)}
										</Show>
									</div>
								</div>
								<div class="formrow">
									<div class="fl">
										<span>Slug</span>
									</div>
									<div class="field">
										<span class="derived">{slugOf(name())}</span>
									</div>
								</div>
								<div class="formrow">
									<div class="fl">
										<label for="np-url">URL</label>
									</div>
									<div class="field">
										<TextInput
											id="np-url"
											type="url"
											value={url()}
											onInput={(event) => setUrl(event.currentTarget.value)}
											autocomplete="url"
										/>
									</div>
								</div>
								<div class="formrow">
									<div class="fl">
										<span id="np-vis">Visibility</span>
									</div>
									<div class="field">
										<div
											class="ui-segmented"
											role="radiogroup"
											aria-labelledby="np-vis"
											style={{ "align-self": "flex-start" }}
										>
											{visibilityOption("public", "Public")}
											{visibilityOption("private", "Private")}
										</div>
									</div>
								</div>
							</div>
							<div class="setcard-foot">
								<Show when={failed()}>
									{(failure) => (
										<p class="ferror" role="alert">
											<Truncate
												text={failureOf(
													failure().error,
													"Bencher did not make the project",
												)}
												reveal
											/>
										</p>
									)}
								</Show>
								<span class="spacer" />
								<Button onClick={() => navigate("/organizations")}>
									Cancel
								</Button>
								<Button
									type="submit"
									variant="primary"
									disabled={
										!name().trim() || problem() !== undefined || creating()
									}
								>
									Create project
								</Button>
							</div>
						</form>
					)}
				</Show>
			</div>
		</main>
	);
};

export default NewProject;
