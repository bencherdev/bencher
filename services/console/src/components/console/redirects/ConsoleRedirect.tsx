import * as Sentry from "@sentry/astro";
import { createMemo, createResource, onMount } from "solid-js";
import type { JsonConsoleUser } from "../../../types/bencher";
import { signInHref } from "../../../next/reader";
import { authUser } from "../../../util/auth";
import { httpGet } from "../../../util/http";
import { removeOnboardProjectKey } from "../../../util/onboard";
import {
	NotifyKind,
	forwardParams,
	navigateNotify,
} from "../../../util/notify";
import { getOrganization, setOrganization } from "../../../util/organization";
import { useNavigate, useSearchParams } from "../../../util/url";
import {
	type InitValid,
	init_valid,
	validPlanLevel,
} from "../../../util/valid";
import { ORGANIZATIONS_PATH, organizationPath } from "../../../next/paths";
import { PLAN_PARAM } from "../../auth/auth";
import { consoleDestination } from "./destination";

const ConsoleRedirect = () => {
	onMount(() => removeOnboardProjectKey());
	const [bencher_valid] = createResource(init_valid);
	const user = authUser();
	const navigate = useNavigate();
	const [searchParams] = useSearchParams();

	// Arriving with a plan selected (e.g. from /pricing) means the visitor wants
	// to sign up for that plan, so route them to organization billing instead of
	// projects. New users with no projects still go through onboarding below,
	// which forwards the plan param to its final plan step.
	const orgDestination = (slug: string) =>
		validPlanLevel(searchParams[PLAN_PARAM])
			? forwardParams(`${ORGANIZATIONS_PATH}/${slug}/billing`, [PLAN_PARAM], [])
			: organizationPath(slug);

	const help = () => {
		navigateNotify(
			NotifyKind.ERROR,
			"Failed to load Bencher Console. This may be a bug. Please try again or contact support.",
			`/console/users/${user?.slug}/help`,
			[PLAN_PARAM],
			[],
			true,
		);
	};

	const fetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
		};
	});
	const redirect = async (fetcher: { bencher_valid: InitValid }) => {
		if (!fetcher.bencher_valid) {
			return;
		}
		if (!user) {
			location.replace(signInHref(`${location.pathname}${location.search}`));
			return;
		}
		const path = `/v0/console/users/${user.uuid}`;
		return await httpGet(path)
			.then((resp) => {
				const reader: JsonConsoleUser = resp?.data;
				const destination = consoleDestination(
					reader,
					getOrganization()?.slug,
					validPlanLevel(searchParams[PLAN_PARAM]),
				);
				const only =
					reader.organizations.length === 1
						? reader.organizations[0]
						: undefined;
				if (only) {
					setOrganization(only);
				}
				switch (destination.kind) {
					case "project":
						navigate(destination.path, { replace: true });
						break;
					case "organization":
						navigate(orgDestination(destination.slug), { replace: true });
						break;
					case "organizations":
						navigate(ORGANIZATIONS_PATH, { replace: true });
						break;
					case "onboard":
						navigate(
							forwardParams("/console/onboard/project", [PLAN_PARAM], []),
							{ replace: true },
						);
						break;
					case "help":
						help();
						break;
				}
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				help();
			});
	};
	createResource(fetcher, redirect);

	return null;
};

export default ConsoleRedirect;
