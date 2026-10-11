import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { initSync, scale_factor, scale_units_symbol } from "bencher_valid";
import { describe, expect, test } from "vitest";
import { unitScale } from "./units";

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

const MINIMUMS = [
	Number.NaN,
	Number.NEGATIVE_INFINITY,
	-1,
	0,
	0.5,
	59,
	60,
	999,
	1000,
	3599,
	3600,
	999_999,
	1e6,
	1e9,
	59_999_999_999,
	60e9,
	3.6e12,
	1e12,
	1e15,
	1e18,
	Number.POSITIVE_INFINITY,
];

const UNITS = [
	"nanoseconds (ns)",
	"seconds (s)",
	"bytes (B)",
	"percentage (%)",
	"decibels (dB)",
	"instructions",
	"widgets (w)",
	"a (b) (c)",
	"a )b( c",
	"(",
	"",
];

// Kills any drift between this port and the Rust the classic console runs as WASM.
describe.skipIf(!wasmReady)("unitScale matches bencher_valid", () => {
	test.each(UNITS)("for %j", (units) => {
		for (const min of MINIMUMS) {
			expect(unitScale(min, units), `${min} ${units}`).toEqual({
				factor: Number(scale_factor(min, units)),
				symbol: scale_units_symbol(min, units),
			});
		}
	});
});
