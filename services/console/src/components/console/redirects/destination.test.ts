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
	// Kills picking the first or the last project instead of the newest report.
	test("lands on the version 1 project with the newest report", () => {
		const reader = {
			organizations: [acme],
			projects: [
				project("older", acme, 1, 1_000),
				project("newest", acme, 1, 3_000),
				project("unrun", acme, 1),
				project("old", acme, 0, 2_000),
			],
		};
		expect(consoleDestination(reader, undefined, false)).toEqual({
			kind: "next",
			path: "/next/console/projects/newest/",
		});
	});

	// Kills choosing among version 1 projects only, or sending a version 0 project to the new console.
	test("lands on a version 0 project's classic page when its report is newest", () => {
		const reader = {
			organizations: [acme],
			projects: [
				project("next", acme, 1, 1_000),
				project("classic", acme, 0, 2_000),
			],
		};
		expect(consoleDestination(reader, undefined, false)).toEqual({
			kind: "classic",
			path: "/console/projects/classic/plots",
		});
	});

	// Kills landing on a project when the reader has no version 1 project.
	test("keeps today's destinations with no version 1 project", () => {
		const reader = {
			organizations: [acme],
			projects: [project("classic", acme, 0, 2_000)],
		};
		expect(consoleDestination(reader, undefined, false)).toEqual({
			kind: "organization",
			slug: "acme",
		});
		expect(consoleDestination(reader, "cached", false)).toEqual({
			kind: "organization",
			slug: "cached",
		});
		expect(
			consoleDestination(
				{ organizations: [acme, other], projects: reader.projects },
				undefined,
				false,
			),
		).toEqual({ kind: "organizations" });
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

	// Kills landing on a project with no report, or dropping a plan the reader came to buy.
	test("keeps today's destinations with no report or with a plan", () => {
		const unrun = {
			organizations: [acme],
			projects: [project("unrun", acme, 1)],
		};
		expect(consoleDestination(unrun, undefined, false)).toEqual({
			kind: "organization",
			slug: "acme",
		});
		const run = {
			organizations: [acme],
			projects: [project("next", acme, 1, 1_000)],
		};
		expect(consoleDestination(run, undefined, true)).toEqual({
			kind: "organization",
			slug: "acme",
		});
	});
});
