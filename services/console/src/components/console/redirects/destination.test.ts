import { describe, expect, test } from "vitest";
import type {
	JsonConsoleOrganization,
	JsonConsoleUserProject,
} from "../../../types/bencher";
import { consoleDestination } from "./destination";

const org = (slug: string): JsonConsoleOrganization => ({
	uuid: `${slug}-uuid`,
	name: slug,
	slug,
});

const project = (
	slug: string,
	organization: JsonConsoleOrganization,
	bmf_version: number,
	last_report?: number,
): JsonConsoleUserProject => ({
	uuid: `${slug}-uuid`,
	name: slug,
	slug,
	organization: organization.uuid,
	bmf_version,
	...(last_report === undefined ? {} : { last_report }),
	active_alerts: 0,
});

const acme = org("acme");
const other = org("other");

describe("consoleDestination", () => {
	// Kills picking the first or the last project instead of the newest report,
	// and a version 0 project passed over or sent anywhere but the console.
	test("lands on the project with the newest report at either version", () => {
		const reader = {
			organizations: [acme],
			projects: [
				project("older", acme, 1, 1_000),
				project("newest", acme, 0, 3_000),
				project("unrun", acme, 1),
				project("old", acme, 1, 2_000),
			],
		};
		expect(consoleDestination(reader, undefined, false)).toEqual({
			kind: "project",
			path: "/console/projects/newest/",
		});
	});

	// Kills a reader whose projects never ran sent to an organization's page,
	// or a cached organization taking precedence over a project.
	test("lands on a project that never ran too", () => {
		const reader = {
			organizations: [acme, other],
			projects: [project("unrun", acme, 0)],
		};
		expect(consoleDestination(reader, "cached", false)).toEqual({
			kind: "project",
			path: "/console/projects/unrun/",
		});
	});

	// Kills a reader with no projects or no organizations sent anywhere but onboarding or help.
	test("onboards a reader with no projects", () => {
		expect(
			consoleDestination(
				{ organizations: [acme], projects: [] },
				undefined,
				false,
			),
		).toEqual({ kind: "onboard" });
		expect(
			consoleDestination({ organizations: [], projects: [] }, undefined, false),
		).toEqual({ kind: "help" });
	});

	// Kills dropping a plan the reader came to buy, or the organization it is for.
	test("sends a reader with a plan to an organization", () => {
		const run = {
			organizations: [acme],
			projects: [project("next", acme, 1, 1_000)],
		};
		expect(consoleDestination(run, undefined, true)).toEqual({
			kind: "organization",
			slug: "acme",
		});
		expect(consoleDestination(run, "cached", true)).toEqual({
			kind: "organization",
			slug: "cached",
		});
		expect(
			consoleDestination(
				{ organizations: [acme, other], projects: run.projects },
				undefined,
				true,
			),
		).toEqual({ kind: "organizations" });
	});
});
