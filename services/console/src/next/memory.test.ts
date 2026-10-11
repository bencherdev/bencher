import { describe, expect, test } from "vitest";
import {
	SHELL_KEY,
	VERSION_KEY,
	forgetShell,
	readShell,
	rememberShell,
	rememberVersion,
	rememberedVersion,
} from "./memory";

const storage = (entries: Record<string, string> = {}) => {
	const map = new Map(Object.entries(entries));
	return {
		getItem: (key: string) => map.get(key) ?? null,
		setItem: (key: string, value: string) => {
			map.set(key, value);
		},
		removeItem: (key: string) => {
			map.delete(key);
		},
		map,
	};
};

describe("version memory", () => {
	// Kills a memory keyed by anything but the slug, and a write that drops
	// the other projects.
	test("remembers each project's version by slug", () => {
		const store = storage();
		rememberVersion(store, "hashbrown", 1);
		rememberVersion(store, "crinkle-cut", 0);
		expect(rememberedVersion(store, "hashbrown")).toBe(1);
		expect(rememberedVersion(store, "crinkle-cut")).toBe(0);
		expect(rememberedVersion(store, "tater-tot")).toBeUndefined();
	});

	// Kills a stale entry that a newer load cannot correct.
	test("the latest load wins", () => {
		const store = storage();
		rememberVersion(store, "hashbrown", 0);
		rememberVersion(store, "hashbrown", 1);
		expect(rememberedVersion(store, "hashbrown")).toBe(1);
	});

	// Kills a memory that throws, or keeps junk, when another script wrote it.
	test("starts over from anything it cannot read", () => {
		const store = storage({ [VERSION_KEY]: "not json" });
		expect(rememberedVersion(store, "hashbrown")).toBeUndefined();
		rememberVersion(store, "hashbrown", 1);
		expect(JSON.parse(store.map.get(VERSION_KEY) ?? "")).toEqual({
			hashbrown: 1,
		});
		expect(
			rememberedVersion(
				storage({ [VERSION_KEY]: '{"hashbrown":7}' }),
				"hashbrown",
			),
		).toBeUndefined();
	});

	// Kills a memory that grows with every project a reader ever opened, and
	// one that forgets by first write instead of last load.
	test("keeps only the most recent projects", () => {
		const store = storage();
		for (let i = 0; i < 32; i++) {
			rememberVersion(store, `project-${i}`, 1);
		}
		rememberVersion(store, "project-0", 1);
		rememberVersion(store, "project-32", 0);
		expect(
			Object.keys(JSON.parse(store.map.get(VERSION_KEY) ?? "")),
		).toHaveLength(32);
		expect(rememberedVersion(store, "project-0")).toBe(1);
		expect(rememberedVersion(store, "project-1")).toBeUndefined();
		expect(rememberedVersion(store, "project-32")).toBe(0);
	});
});

// Kills a refused write that throws out of the page that loaded the project.
test("a browser that refuses storage keeps working without the memory", () => {
	const refusing = {
		getItem: () => null,
		setItem: () => {
			throw new DOMException("refused", "QuotaExceededError");
		},
		removeItem: () => {},
	};
	expect(() => rememberVersion(refusing, "hashbrown", 1)).not.toThrow();
	expect(() =>
		rememberShell(refusing, "reader-a", "hashbrown", {
			organization: "Pompeii LLC",
			organizationUuid: "6b2f3b5e-0c4c-4a64-9a43-5a1f0b3f1d2e",
			name: "Hashbrown",
			alerts: 1,
		}),
	).not.toThrow();
});

describe("shell memory", () => {
	const shell = {
		organization: "Pompeii LLC",
		organizationUuid: "6b2f3b5e-0c4c-4a64-9a43-5a1f0b3f1d2e",
		name: "Hashbrown",
		alerts: 1,
	};

	// Kills one reader painting another reader's project names.
	test("belongs to the reader who wrote it", () => {
		const store = storage();
		rememberShell(store, "reader-a", "hashbrown", shell);
		expect(readShell(store, "reader-a", "hashbrown")).toEqual(shell);
		expect(readShell(store, "reader-b", "hashbrown")).toBeUndefined();
	});

	// Kills a write by a new reader that keeps the last reader's projects.
	test("a new reader starts from nothing", () => {
		const store = storage();
		rememberShell(store, "reader-a", "hashbrown", shell);
		rememberShell(store, "reader-b", "tater-tot", {
			...shell,
			name: "Tater Tot",
		});
		expect(JSON.parse(store.map.get(SHELL_KEY) ?? "").projects).toEqual({
			"tater-tot": { ...shell, name: "Tater Tot" },
		});
	});

	// Kills a memory that grows with every project a reader ever opened.
	test("keeps only the most recent projects", () => {
		const store = storage();
		for (let i = 0; i < 40; i++) {
			rememberShell(store, "reader-a", `project-${i}`, shell);
		}
		rememberShell(store, "reader-a", "project-5", shell);
		const projects = Object.keys(
			JSON.parse(store.map.get(SHELL_KEY) ?? "").projects,
		);
		expect(projects).toHaveLength(32);
		expect(projects.at(-1)).toBe("project-5");
		expect(projects).not.toContain("project-0");
	});

	// Kills a sign out that leaves names behind.
	test("is forgotten", () => {
		const store = storage();
		rememberShell(store, "reader-a", "hashbrown", shell);
		forgetShell(store);
		expect(store.map.has(SHELL_KEY)).toBe(false);
	});
});
