import { describe, expect, test } from "vitest";
import {
	base64ToBytes,
	bytesToBase64,
	decodeBase64,
	encodeBase64,
} from "./base64";

describe("base64 round-trip", () => {
	test("encodeBase64 and decodeBase64 are inverses", () => {
		const input = "Hello, World!";
		expect(decodeBase64(encodeBase64(input))).toBe(input);
	});

	test("handles unicode text", () => {
		const input = "Unicode: éèê";
		expect(decodeBase64(encodeBase64(input))).toBe(input);
	});

	test("base64ToBytes returns Uint8Array", () => {
		const bytes = base64ToBytes(btoa("abc"));
		expect(bytes).toBeInstanceOf(Uint8Array);
		expect(bytes.length).toBe(3);
	});

	test("bytesToBase64 encodes bytes", () => {
		const bytes = new TextEncoder().encode("test");
		const b64 = bytesToBase64(bytes);
		expect(atob(b64)).toBe("test");
	});
});
