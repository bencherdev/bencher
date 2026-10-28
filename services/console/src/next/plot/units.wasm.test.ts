import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { initSync, scale_factor, scale_units_symbol } from "bencher_valid";
import { describe, expect, test } from "vitest";
import { axisTier } from "./format";

const WASM = fileURLToPath(
	new URL(
		"../../../../../lib/bencher_valid/pkg/bencher_valid_bg.wasm",
		import.meta.url,
	),
);
const wasmReady = existsSync(WASM);
if (wasmReady) {
	initSync({ module: readFileSync(WASM) });
}

// Inside every tier and away from a rounding carry, where the console and the Rust tiers agree by design.
const SIZES = [
	0.5, 1, 59, 60, 999, 1000, 3599, 3600, 1e6, 1e9, 60e9, 3.6e12, 1e12, 1e15,
];

// Seconds are left out: the console reads them in the nanosecond tiers, the Rust from a second up.
const UNITS = ["nanoseconds (ns)", "bytes (B)"];

// Kills any drift between these tiers and the Rust the classic console runs as WASM.
describe.skipIf(!wasmReady)("axisTier matches bencher_valid", () => {
	test.each(UNITS)("for %j", (units) => {
		for (const size of SIZES) {
			expect(axisTier(size, units), `${size} ${units}`).toEqual({
				factor: Number(scale_factor(size, units)),
				symbol: scale_units_symbol(size, units),
				exponent: 0,
			});
		}
	});
});
