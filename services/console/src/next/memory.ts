// Both memories are read by inline scripts before any module loads, so their
// keys and shapes are a contract with `NextHead.astro`, `ShellPaint.astro`, and
// the classic console.

type Storage = Pick<globalThis.Storage, "getItem" | "setItem" | "removeItem">;

/** Each project's BMF version by slug, written whenever either console loads one. */
export const VERSION_KEY = "BENCHER_BMF_VERSIONS";

export type BmfVersion = 0 | 1;

export const rememberedVersion = (
	storage: Pick<Storage, "getItem">,
	slug: string,
): BmfVersion | undefined => {
	const version = readJson(storage, VERSION_KEY)?.[slug];
	return version === 0 || version === 1 ? version : undefined;
};

export const rememberVersion = (
	storage: Storage,
	slug: string,
	version: BmfVersion,
) => {
	const versions = readJson(storage, VERSION_KEY) ?? {};
	write(storage, VERSION_KEY, recent(versions, slug, version));
};

/** What the shell shows for a project, so a returning reader's first paint has it. */
export const SHELL_KEY = "BENCHER_CONSOLE_SHELL";

export interface ShellMemory {
	organization: string;
	organizationUuid: string;
	name: string;
	alerts: number;
}

type ShellRecord = {
	reader: string;
	projects: Record<string, ShellMemory>;
};

export const readShell = (
	storage: Pick<Storage, "getItem">,
	reader: string,
	slug: string,
): ShellMemory | undefined => {
	const record = readJson(storage, SHELL_KEY) as ShellRecord | undefined;
	return record?.reader === reader ? record.projects?.[slug] : undefined;
};

export const rememberShell = (
	storage: Storage,
	reader: string,
	slug: string,
	shell: ShellMemory,
) => {
	const record = readJson(storage, SHELL_KEY) as ShellRecord | undefined;
	const projects = record?.reader === reader ? (record.projects ?? {}) : {};
	write(storage, SHELL_KEY, {
		reader,
		projects: recent(projects, slug, shell),
	});
};

export const forgetShell = (storage: Pick<Storage, "removeItem">) =>
	storage.removeItem(SHELL_KEY);

const RECENT_PROJECTS = 32;

/** The most recent projects, with `slug` just seen. */
const recent = <T>(projects: Record<string, T>, slug: string, value: T) => {
	const copy = { ...projects };
	// Insertion order is recency: the project just seen moves to the end.
	delete copy[slug];
	copy[slug] = value;
	return Object.fromEntries(Object.entries(copy).slice(-RECENT_PROJECTS));
};

// A browser can refuse storage; the consoles then work without their memory.
const write = (
	storage: Pick<Storage, "setItem">,
	key: string,
	value: unknown,
) => {
	try {
		storage.setItem(key, JSON.stringify(value));
	} catch {
		// Nothing is remembered, and nothing else changes.
	}
};

const readJson = (
	storage: Pick<Storage, "getItem">,
	key: string,
): Record<string, unknown> | undefined => {
	try {
		const value = JSON.parse(storage.getItem(key) ?? "null");
		return value && typeof value === "object" ? value : undefined;
	} catch {
		return undefined;
	}
};
