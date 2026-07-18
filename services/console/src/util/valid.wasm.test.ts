// @vitest-environment happy-dom
import { describe, expect, test } from "vitest";
import {
	init_valid,
	jwtOptionalInvalid,
	jwtRequiredInvalid,
	validBenchmarkName,
	validBoundary,
	validBranchName,
	validCdfBoundary,
	validEmail,
	validIqrBoundary,
	validIndex,
	validJwt,
	validModel,
	validNonEmpty,
	validOptionJwt,
	validOptionResourceName,
	validOptionUrl,
	validOptionUuid,
	validPercentageBoundary,
	validPlanLevel,
	validResourceName,
	validSampleSize,
	validSlug,
	validUser,
	validUserName,
	validUuid,
	validWindow,
} from "./valid";

const wasmReady = await init_valid();

describe.skipIf(!wasmReady)("WASM validators", () => {
	const fakeJwt =
		"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";

	describe("validUuid", () => {
		test("accepts valid UUID", () => {
			expect(validUuid("a1b2c3d4-e5f6-7890-abcd-ef1234567890")).toBe(true);
		});

		test("rejects invalid UUID", () => {
			expect(validUuid("not-a-uuid")).toBe(false);
		});

		test("rejects empty string", () => {
			expect(validUuid("")).toBe(false);
		});
	});

	describe("validOptionUuid", () => {
		test("accepts valid UUID", () => {
			expect(validOptionUuid("a1b2c3d4-e5f6-7890-abcd-ef1234567890")).toBe(
				true,
			);
		});

		test("rejects null", () => {
			expect(validOptionUuid(null)).toBe(false);
		});

		test("rejects undefined", () => {
			expect(validOptionUuid(undefined)).toBe(false);
		});
	});

	describe("validUserName", () => {
		test("accepts valid name", () => {
			expect(validUserName("Everett")).toBe(true);
		});

		test("rejects empty string", () => {
			expect(validUserName("")).toBe(false);
		});
	});

	describe("validResourceName", () => {
		test("accepts valid resource name", () => {
			expect(validResourceName("My Project")).toBe(true);
		});

		test("rejects empty string", () => {
			expect(validResourceName("")).toBe(false);
		});
	});

	describe("validOptionResourceName", () => {
		test("accepts valid resource name", () => {
			expect(validOptionResourceName("My Project")).toBe(true);
		});

		test("rejects null", () => {
			expect(validOptionResourceName(null)).toBe(false);
		});

		test("rejects undefined", () => {
			expect(validOptionResourceName(undefined)).toBe(false);
		});
	});

	describe("validBranchName", () => {
		test("accepts valid branch name", () => {
			expect(validBranchName("main")).toBe(true);
			expect(validBranchName("feature/my-feature")).toBe(true);
		});

		test("rejects empty string", () => {
			expect(validBranchName("")).toBe(false);
		});
	});

	describe("validBenchmarkName", () => {
		test("accepts valid benchmark name", () => {
			expect(validBenchmarkName("bench_add")).toBe(true);
		});

		test("rejects empty string", () => {
			expect(validBenchmarkName("")).toBe(false);
		});
	});

	describe("validNonEmpty", () => {
		test("accepts non-empty string", () => {
			expect(validNonEmpty("hello")).toBe(true);
		});

		test("rejects empty string", () => {
			expect(validNonEmpty("")).toBe(false);
		});

		test("rejects whitespace-only string", () => {
			expect(validNonEmpty("   ")).toBe(false);
		});
	});

	describe("validSlug", () => {
		test("accepts valid slug", () => {
			expect(validSlug("my-project")).toBe(true);
			expect(validSlug("project123")).toBe(true);
		});

		test("rejects null", () => {
			expect(validSlug(null)).toBe(false);
		});

		test("rejects undefined", () => {
			expect(validSlug(undefined)).toBe(false);
		});

		test("rejects empty string", () => {
			expect(validSlug("")).toBe(false);
		});

		test("rejects slug with spaces", () => {
			expect(validSlug("my project")).toBe(false);
		});
	});

	describe("validEmail", () => {
		test("accepts valid email", () => {
			expect(validEmail("user@example.com")).toBe(true);
		});

		test("rejects invalid email", () => {
			expect(validEmail("not-an-email")).toBe(false);
		});

		test("rejects null", () => {
			expect(validEmail(null)).toBe(false);
		});

		test("rejects undefined", () => {
			expect(validEmail(undefined)).toBe(false);
		});

		test("rejects empty string", () => {
			expect(validEmail("")).toBe(false);
		});
	});

	describe("validJwt", () => {
		test("accepts valid JWT", () => {
			expect(validJwt(fakeJwt)).toBe(true);
		});

		test("rejects invalid JWT", () => {
			expect(validJwt("not.a.jwt")).toBe(false);
		});

		test("rejects null", () => {
			expect(validJwt(null)).toBe(false);
		});
	});

	describe("validOptionJwt", () => {
		test("rejects null", () => {
			expect(validOptionJwt(null)).toBe(false);
		});

		test("rejects undefined", () => {
			expect(validOptionJwt(undefined)).toBe(false);
		});
	});

	describe("validOptionUrl", () => {
		test("accepts valid URL", () => {
			expect(validOptionUrl("https://bencher.dev")).toBe(true);
		});

		test("rejects null", () => {
			expect(validOptionUrl(null)).toBe(false);
		});
	});

	describe("validUser", () => {
		test("accepts valid user object", () => {
			const user = {
				user: {
					uuid: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
					name: "Test User",
					slug: "test-user",
					email: "test@example.com",
				},
				token: fakeJwt,
			};
			// biome-ignore lint/suspicious/noExplicitAny: partial test object
			expect(validUser(user as any)).toBe(true);
		});

		test("rejects user with invalid email", () => {
			const user = {
				user: {
					uuid: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
					name: "Test User",
					slug: "test-user",
					email: "invalid",
				},
				token: fakeJwt,
			};
			// biome-ignore lint/suspicious/noExplicitAny: partial test object
			expect(validUser(user as any)).toBe(false);
		});
	});

	describe("validWindow", () => {
		test("accepts valid window value", () => {
			expect(validWindow(1)).toBe(true);
			expect(validWindow(10)).toBe(true);
		});

		test("rejects zero", () => {
			expect(validWindow(0)).toBe(false);
		});

		test("rejects undefined", () => {
			expect(validWindow(undefined)).toBe(false);
		});
	});

	describe("validIndex", () => {
		test("accepts valid index", () => {
			expect(validIndex(0)).toBe(true);
			expect(validIndex(1)).toBe(true);
		});

		test("rejects undefined", () => {
			expect(validIndex(undefined)).toBe(false);
		});
	});

	describe("validBoundary", () => {
		test("accepts valid boundary", () => {
			expect(validBoundary("1.5")).toBe(true);
		});

		test("rejects empty string", () => {
			expect(validBoundary("")).toBe(false);
		});
	});

	describe("validPercentageBoundary", () => {
		test("accepts valid percentage boundary", () => {
			expect(validPercentageBoundary("0.5")).toBe(true);
		});

		test("rejects empty string", () => {
			expect(validPercentageBoundary("")).toBe(false);
		});
	});

	describe("validCdfBoundary", () => {
		test("accepts valid CDF boundary", () => {
			expect(validCdfBoundary("0.95")).toBe(true);
		});

		test("rejects empty string", () => {
			expect(validCdfBoundary("")).toBe(false);
		});
	});

	describe("validIqrBoundary", () => {
		test("accepts valid IQR boundary", () => {
			expect(validIqrBoundary("1.5")).toBe(true);
		});

		test("rejects empty string", () => {
			expect(validIqrBoundary("")).toBe(false);
		});
	});

	describe("validSampleSize", () => {
		test("accepts valid sample size", () => {
			expect(validSampleSize("2")).toBe(true);
			expect(validSampleSize("30")).toBe(true);
		});

		test("rejects empty string", () => {
			expect(validSampleSize("")).toBe(false);
		});
	});

	describe("validModel", () => {
		test("rejects null", () => {
			// biome-ignore lint/suspicious/noExplicitAny: testing wrong type
			expect(validModel(null as any)).toBe(false);
		});

		test("rejects non-object", () => {
			// biome-ignore lint/suspicious/noExplicitAny: testing wrong type
			expect(validModel("string" as any)).toBe(false);
		});
	});

	describe("validPlanLevel", () => {
		test("accepts valid plan levels", () => {
			expect(validPlanLevel("free")).toBe(true);
			expect(validPlanLevel("team")).toBe(true);
			expect(validPlanLevel("enterprise")).toBe(true);
		});

		test("rejects invalid level", () => {
			expect(validPlanLevel("premium")).toBe(false);
		});

		test("rejects null", () => {
			expect(validPlanLevel(null)).toBe(false);
		});

		test("rejects undefined", () => {
			expect(validPlanLevel(undefined)).toBe(false);
		});
	});

	describe("jwtRequiredInvalid (best-effort, required token)", () => {
		test("skips when token is missing, regardless of validator", () => {
			expect(jwtRequiredInvalid(undefined, undefined)).toBe(true);
			expect(jwtRequiredInvalid(undefined, "")).toBe(true);
			expect(jwtRequiredInvalid(wasmReady, undefined)).toBe(true);
			expect(jwtRequiredInvalid(wasmReady, "")).toBe(true);
		});

		test("does not reject a malformed token before the validator loads", () => {
			// best-effort: let the request through; the server validates it
			expect(jwtRequiredInvalid(undefined, "not.a.jwt")).toBe(false);
			expect(jwtRequiredInvalid(undefined, fakeJwt)).toBe(false);
		});

		test("rejects a malformed token once the validator has loaded", () => {
			expect(jwtRequiredInvalid(wasmReady, "not.a.jwt")).toBe(true);
		});

		test("allows a well-formed token once the validator has loaded", () => {
			expect(jwtRequiredInvalid(wasmReady, fakeJwt)).toBe(false);
		});
	});

	describe("jwtOptionalInvalid (best-effort, optional token)", () => {
		test("allows a missing token (public access)", () => {
			expect(jwtOptionalInvalid(undefined, undefined)).toBe(false);
			expect(jwtOptionalInvalid(undefined, "")).toBe(false);
			expect(jwtOptionalInvalid(wasmReady, undefined)).toBe(false);
			expect(jwtOptionalInvalid(wasmReady, "")).toBe(false);
		});

		test("does not reject a present token before the validator loads", () => {
			expect(jwtOptionalInvalid(undefined, "not.a.jwt")).toBe(false);
			expect(jwtOptionalInvalid(undefined, fakeJwt)).toBe(false);
		});

		test("rejects a present malformed token once the validator has loaded", () => {
			expect(jwtOptionalInvalid(wasmReady, "not.a.jwt")).toBe(true);
		});

		test("allows a present well-formed token once the validator has loaded", () => {
			expect(jwtOptionalInvalid(wasmReady, fakeJwt)).toBe(false);
		});
	});
});
