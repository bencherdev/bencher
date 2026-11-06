import type { JsonSignInEmail } from "../../types/bencher";

/** Which door a sign-in began at, whose heading it keeps. */
export type Door = "login" | "signup";

/** What a sign-in carries to its proof, from the door's address. */
export interface Carry {
	invite?: string;
	claim?: string;
	plan?: string;
	authorization?: string;
}

/** This tab's sign-in by email: the address it emailed, the door it began at, and what it carries. */
export interface Pending {
	email: string;
	door: Door;
	carry: Carry;
	/** When Resend opens, in milliseconds since the epoch. */
	resendAt?: number;
	/** When the address can get another email, while it is at its cap. */
	cappedUntil?: number;
}

/** The sign-in an email went out for, or the address at its cap, as of `now`. */
export const pendingOf = (
	{ email, resend_after, wait }: JsonSignInEmail,
	door: Door,
	carry: Carry,
	now: number,
): Pending => ({
	email,
	door,
	carry,
	...(resend_after === undefined
		? {}
		: { resendAt: now + resend_after * 1000 }),
	...(wait === undefined ? {} : { cappedUntil: now + wait * 1000 }),
});

const PENDING_KEY = "BENCHER_PENDING_SIGN_IN";
const PREFILL_KEY = "BENCHER_DOOR_EMAIL";
const CARRIED = ["invite", "claim", "plan", "authorization"] as const;

type Store = Pick<Storage, "getItem" | "setItem" | "removeItem">;

export const keepPending = (storage: Store, pending: Pending) => {
	try {
		storage.setItem(PENDING_KEY, JSON.stringify(pending));
	} catch {
		// A tab that keeps nothing starts at the door again.
	}
};

export const readPending = (storage: Store): Pending | undefined => {
	try {
		const kept = JSON.parse(storage.getItem(PENDING_KEY) ?? "null");
		if (
			typeof kept?.email !== "string" ||
			!kept.email ||
			(kept.door !== "login" && kept.door !== "signup")
		) {
			return undefined;
		}
		return {
			email: kept.email,
			door: kept.door,
			carry: carryOf(kept.carry),
			...(time(kept.resendAt) === undefined
				? {}
				: { resendAt: time(kept.resendAt) }),
			...(time(kept.cappedUntil) === undefined
				? {}
				: { cappedUntil: time(kept.cappedUntil) }),
		};
	} catch {
		return undefined;
	}
};

export const forgetPending = (storage: Store) => {
	try {
		storage.removeItem(PENDING_KEY);
	} catch {
		// Nothing was kept.
	}
};

/** The address Use another email takes back to the door, to fill in once. */
export const keepPrefill = (storage: Store, email: string) => {
	try {
		storage.setItem(PREFILL_KEY, email);
	} catch {
		// The door starts empty.
	}
};

export const takePrefill = (storage: Store): string | undefined => {
	try {
		const email = storage.getItem(PREFILL_KEY);
		storage.removeItem(PREFILL_KEY);
		return email || undefined;
	} catch {
		return undefined;
	}
};

/** The door's address for `carry`, so a sign-in begun again carries the same. */
export const doorPath = (door: Door, carry: Carry) => {
	const query = new URLSearchParams();
	for (const name of CARRIED) {
		const value = carry[name];
		if (value) {
			query.set(name, value);
		}
	}
	const search = query.toString();
	return `/auth/${door}${search ? `?${search}` : ""}`;
};

const carryOf = (value: unknown): Carry => {
	const carry: Carry = {};
	if (typeof value !== "object" || value === null) {
		return carry;
	}
	for (const name of CARRIED) {
		const field = Reflect.get(value, name);
		if (typeof field === "string" && field) {
			carry[name] = field;
		}
	}
	return carry;
};

const time = (value: unknown) =>
	typeof value === "number" && Number.isFinite(value) ? value : undefined;
