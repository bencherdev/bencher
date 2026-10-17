import type { JsonUsage } from "../../types/bencher";
import type { Api } from "../api";

/** Whether the API would make a private project for the organization `usage` describes. */
export const allowsPrivate = (usage: JsonUsage) => {
	const status: string | undefined = usage.plan?.status;
	switch (usage.kind as string) {
		case "cloud_metered":
			return status === "active" || status === "trialing";
		case "self_hosted_licensed":
			return (
				usage.license !== undefined &&
				(usage.metrics ?? 0) < usage.license.entitlements
			);
		default:
			return false;
	}
};

/** Only the answer is cached, never the plan or its card. */
export const privateQuery = (api: Api, organization: string) => ({
	queryKey: ["organization", organization, "private"],
	queryFn: async ({ signal }: { signal: AbortSignal }) =>
		allowsPrivate(
			(
				await api.get<JsonUsage>(
					`/v0/organizations/${encodeURIComponent(organization)}/usage`,
					signal,
				)
			).data,
		),
});
