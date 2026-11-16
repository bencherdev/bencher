import type { JsonUsage } from "../../types/bencher";
import type { Api } from "../api";

/**
 * Whether deleting the organization would end a contract: its plan is
 * Enterprise and still live in Stripe, as the API's own rule has it.
 */
export const endsAContract = ({ plan }: Pick<JsonUsage, "plan">): boolean => {
	// Compared as text, so the generated enums stay out of the page.
	const level: string | undefined = plan?.level;
	const status: string | undefined = plan?.status;
	return (
		level === "enterprise" &&
		status !== "canceled" &&
		status !== "incomplete_expired"
	);
};

/**
 * Whether deleting the organization with `uuid` would end a contract, read
 * once the reader starts a delete. The cache keeps only the answer, never the
 * plan's payer.
 */
export const contractQuery = (api: Api, uuid: string) => ({
	queryKey: ["organization", uuid, "contract"],
	queryFn: async ({ signal }: { signal: AbortSignal }): Promise<boolean> =>
		endsAContract(
			(await api.get<JsonUsage>(`/v0/organizations/${uuid}/usage`, signal))
				.data,
		),
	staleTime: 0,
});
