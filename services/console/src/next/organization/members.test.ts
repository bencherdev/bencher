import { QueryClient } from "@tanstack/solid-query";
import { describe, expect, test } from "vitest";
import { type JsonMember, OrganizationRole } from "../../types/bencher";
import type { Api } from "../api";
import {
	type MemberPage,
	membersQuery,
	removeMember,
	rolesQuery,
	sendable,
} from "./members";

const member = (uuid: string, name: string): JsonMember => ({
	uuid,
	name,
	slug: name.toLowerCase(),
	email: `${name.toLowerCase()}@nowhere.example`,
	role: OrganizationRole.Leader,
	created: "2026-09-01T00:00:00Z",
	modified: "2026-09-01T00:00:00Z",
});
const MURIEL = member("u1", "Muriel");
const EUSTACE = member("u2", "Eustace");
const signal = new AbortController().signal;

describe("membersQuery", () => {
	// Kills a count taken from the page's length rather than the API's total.
	test("counts every member the API has, past one page", async () => {
		const api = {
			get: async <T>() => ({
				data: [MURIEL] as T,
				headers: new Headers({ "X-Total-Count": "300" }),
			}),
		} as unknown as Api;
		expect(await membersQuery(api, "pompeii-llc").queryFn({ signal })).toEqual({
			members: [MURIEL],
			total: 300,
		});
	});
});

describe("rolesQuery", () => {
	// Kills invite and remove read from the wrong permission, or swapped.
	test("reads invite and remove each from its own permission", async () => {
		const api = {
			get: async <T>(path: string) => ({
				data: { allowed: path.endsWith("/allowed/create_role") } as T,
				headers: new Headers(),
			}),
		} as unknown as Api;
		expect(await rolesQuery(api, "pompeii-llc").queryFn({ signal })).toEqual({
			invite: true,
			remove: false,
		});
	});
});

describe("removeMember", () => {
	const setup = (send: Api["send"]) => {
		const sent: string[] = [];
		const api = {
			get: () => new Promise(() => {}),
			send: (async (method: string, path: string) => {
				sent.push(`${method} ${path}`);
				return send(method as "DELETE", path);
			}) as Api["send"],
		} as Api;
		const client = new QueryClient();
		const key = membersQuery(api, "pompeii-llc").queryKey;
		client.setQueryData<MemberPage>(key, {
			members: [MURIEL, EUSTACE],
			total: 2,
		});
		return { api, client, sent, page: () => client.getQueryData(key) };
	};

	// Kills a remove sent for another member or organization, a list that keeps
	// the removed row or drops another, and a count left as it was.
	test("drops only the removed member once the API agrees", async () => {
		const { api, client, sent, page } = setup(async () => ({
			data: undefined as never,
			headers: new Headers(),
		}));
		await removeMember(api, client, "pompeii-llc", "u2");
		expect(sent).toEqual(["DELETE /v0/organizations/pompeii-llc/members/u2"]);
		expect(page()).toEqual({ members: [MURIEL], total: 1 });
	});

	// Kills a remove that fails once the API agreed because no list was cached,
	// and one that caches a list it never read.
	test("a remove with no list cached leaves the cache empty", async () => {
		const { api, client, page } = setup(async () => ({
			data: undefined as never,
			headers: new Headers(),
		}));
		client.clear();
		await removeMember(api, client, "pompeii-llc", "u2");
		expect(page()).toBeUndefined();
	});

	// Kills a row dropped before the API agreed.
	test("keeps the row when the API refuses", async () => {
		const { api, client, page } = setup(async () => {
			throw new Error("refused");
		});
		await expect(
			removeMember(api, client, "pompeii-llc", "u2"),
		).rejects.toThrow();
		expect(page()).toEqual({ members: [MURIEL, EUSTACE], total: 2 });
	});
});

describe("sendable", () => {
	// Kills Send invite offered for an address with nothing before its `@`, or none.
	test("needs a name before the @", () => {
		expect(sendable("courage@nowhere.example")).toBe(true);
		expect(sendable("@nowhere.example")).toBe(false);
		expect(sendable("courage")).toBe(false);
	});
});
