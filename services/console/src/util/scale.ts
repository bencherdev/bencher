import {
	scale_factor as wasm_scale_factor,
	scale_units as wasm_scale_units,
	scale_units_symbol as wasm_scale_units_symbol,
} from "bencher_valid";

export const scale_factor = (min: number, raw_units: string) => {
	if (typeof min === "number" && typeof raw_units === "string") {
		try {
			const factor = Number(wasm_scale_factor(min, raw_units));
			if (Number.isNaN(factor)) {
				console.error("Conversion to number failed:", factor);
			} else {
				return factor;
			}
		} catch (error) {
			console.error(
				"Failed to convert scale factor to number:",
				min,
				raw_units,
				error,
			);
		}
	}
	return 1;
};

export const scale_units = (min: number, raw_units: string) => {
	if (typeof min === "number" && typeof raw_units === "string") {
		try {
			const units = wasm_scale_units(min, raw_units);
			if (typeof units === "string") {
				return units;
			}
		} catch (error) {
			console.error(
				"Failed to convert scale units to string:",
				min,
				raw_units,
				error,
			);
		}
	}
	return "Units";
};

export const scale_units_symbol = (min: number, raw_units: string) => {
	if (typeof min === "number" && typeof raw_units === "string") {
		try {
			const units_symbol = wasm_scale_units_symbol(min, raw_units);
			if (typeof units_symbol === "string") {
				return units_symbol;
			}
		} catch (error) {
			console.error(
				"Failed to convert scale units symbol to string:",
				min,
				raw_units,
				error,
			);
		}
	}
	return "";
};
