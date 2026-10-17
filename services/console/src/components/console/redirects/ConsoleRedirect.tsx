import * as Sentry from "@sentry/astro";
import { createMemo, createResource, onMount } from "solid-js";
import type { JsonConsoleUser } from "../../../types/bencher";
import { authUser, removeUser } from "../../../util/auth";
import { httpGet } from "../../../util/http";
import { removeOnboardProjectKey } from "../../../util/onboard";
import {
	NOTIFY_KIND_PARAM,
	NOTIFY_TEXT_PARAM,
	NotifyKind,
	forwardParams,
	navigateNotify,
} from "../../../util/notify";
import {
	getOrganization,
	removeOrganization,
	setOrganization,
} from "../../../util/organization";
import { useNavigate, useSearchParams } from "../../../util/url";
import {
	type InitValid,
	init_valid,
	validJwt,
	validPlanLevel,
} from "../../../util/valid";
import { PLAN_PARAM } from "../../auth/auth";
import { consoleDestination } from "./destination";

export interface Props {
	apiUrl: string;
}

const ConsoleRedirect = (props: Props) => {
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
			? forwardParams(
					`/console/organizations/${slug}/billing`,
					[PLAN_PARAM],
					[],
				)
			: forwardParams(
					`/console/organizations/${slug}/projects`,
					[NOTIFY_KIND_PARAM, NOTIFY_TEXT_PARAM],
					null,
				);

	const logout = () => {
		removeUser();
		removeOrganization();
		navigate("/auth/login", { replace: true });
	};
	const help = () => {
		navigateNotify(
			NotifyKind.ERROR,
			"Failed to load Bencher Console. This may be a bug. Please try again or contact support.",
			`/console/users/${user?.user?.slug}/help`,
			[PLAN_PARAM],
			[],
			true,
		);
	};

	const fetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			token: user.token,
		};
	});
	const redirect = async (fetcher: {
		bencher_valid: InitValid;
		token: string;
	}) => {
		if (!fetcher.bencher_valid) {
			return;
		}
		if (!validJwt(fetcher.token)) {
			logout();
			return;
		}
		const path = `/v0/console/users/${user.user.uuid}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
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
					case "next":
						navigate(destination.path, { replace: true });
						break;
					case "classic":
						navigate(
							forwardParams(
								destination.path,
								[NOTIFY_KIND_PARAM, NOTIFY_TEXT_PARAM],
								null,
							),
							{ replace: true },
						);
						break;
					case "organization":
						navigate(orgDestination(destination.slug), { replace: true });
						break;
					case "organizations":
						navigate("/console/organizations", { replace: true });
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

	return <></>;
};

export default ConsoleRedirect;
