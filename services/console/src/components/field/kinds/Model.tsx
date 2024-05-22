import { For, createMemo, createSignal } from "solid-js";
import { ModelTest } from "../../../types/bencher";
import {
	validBoundary,
	validCdfBoundary,
	validIqrBoundary,
	validPercentageBoundary,
	validSampleSize,
	validU32,
} from "../../../util/valid";
import type { FieldConfig, FieldHandler, FieldValue } from "../Field";
import FieldKind from "../kind";
import { createStore } from "solid-js/store";
import Field from "../Field";

export type InputValue = string | number | null | undefined;

export interface Props {
	value: FieldValue;
	valid: undefined | null | boolean;
	config: FieldConfig;
	handleField: FieldHandler;
}

export interface StatisticConfig {
	icon: string;
	type: string;
	placeholder?: string;
	value: InputValue;
	disabled?: boolean;
	help?: string;
	validate: (value: InputValue) => boolean;
}

const STATISTIC_FIELDS = {
	test: {
		icon: "fas fa-vial",
	},
	static_lower_boundary: {
		type: "input",
		placeholder: "50",
		icon: "fas fa-arrow-down",
		help: "Must be any finite floating point number",
		validate: validBoundary,
	},
	static_upper_boundary: {
		type: "input",
		placeholder: "100",
		icon: "fas fa-arrow-up",
		help: "Must be any finite floating point number",
		validate: validBoundary,
	},
	percentage_lower_boundary: {
		type: "input",
		placeholder: "0.50",
		icon: "fas fa-arrow-down",
		help: "Must be any percentage greater than or equal to zero in decimal form",
		validate: validPercentageBoundary,
	},
	percentage_upper_boundary: {
		type: "input",
		placeholder: "0.50",
		icon: "fas fa-arrow-up",
		help: "Must be any percentage greater than or equal to zero in decimal form",
		validate: validPercentageBoundary,
	},
	cdf_lower_boundary: {
		type: "input",
		placeholder: "0.98",
		icon: "fas fa-arrow-down",
		help: "Must be between 0.50 and 1.00 (lower is stricter; higher is looser)",
		validate: validCdfBoundary,
	},
	cdf_upper_boundary: {
		type: "input",
		placeholder: "0.98",
		icon: "fas fa-arrow-up",
		help: "Must be between 0.50 and 1.00 (lower is stricter; higher is looser)",
		validate: validCdfBoundary,
	},
	iqr_lower_boundary: {
		type: "input",
		placeholder: "3.0",
		icon: "fas fa-arrow-down",
		help: "Must be any multiplier greater than or equal to zero",
		validate: validIqrBoundary,
	},
	iqr_upper_boundary: {
		type: "input",
		placeholder: "3.0",
		icon: "fas fa-arrow-up",
		help: "Must be any multiplier greater than or equal to zero",
		validate: validIqrBoundary,
	},
	min_sample_size: {
		type: "number",
		placeholder: "30",
		icon: "fas fa-cube",
		help: "Must be an integer greater than or equal to 2",
		validate: validSampleSize,
	},
	max_sample_size: {
		type: "number",
		placeholder: "30",
		icon: "fas fa-cubes",
		help: "Must be an integer greater than or equal to 2",
		validate: validSampleSize,
	},
	window: {
		type: "number",
		placeholder: "525600",
		icon: "fas fa-calendar-week",
		help: "Must be an integer greater than zero",
		validate: validU32,
	},
};

const testValue = (selected: ModelTest) => {
	return {
		selected,
		options: [
			{
				value: ModelTest.Static,
				option: "Static",
			},
			{
				value: ModelTest.Percentage,
				option: "Percentage",
			},
			{
				value: ModelTest.ZScore,
				option: "z-score",
			},
			{
				value: ModelTest.TTest,
				option: "t-test",
			},
			{
				value: ModelTest.LogNormal,
				option: "Log Normal",
			},
			{
				value: ModelTest.Iqr,
				option: "Interquartile Range (IQR)",
			},
			{
				value: ModelTest.DeltaIqr,
				option: "Delta Interquartile Range (ΔIQR)",
			},
		],
	};
};

const testSelectConfig = (modelTest: ModelTest) => {
	return {
		kind: FieldKind.SELECT,
		label: (
			<div class="level is-mobile">
				<div class="level-left">
					<p class="level-item">Threshold Model</p>
					<a
						class="level-item"
						href={`https://bencher.dev/docs/explanation/thresholds/#${testFragment(
							modelTest,
						)}`}
						// biome-ignore lint/a11y/noBlankTarget: <explanation>
						target="_blank"
						title="Open documentation in new tab"
					>
						<span class="icon">
							<i class="fas fa-book-open" />
						</span>
					</a>
				</div>
			</div>
		),
		key: "test",
		value: testValue(modelTest),
		validate: false,
		config: STATISTIC_FIELDS.test,
	};
};

const testFragment = (modelTest: ModelTest) => {
	switch (modelTest) {
		case ModelTest.Static:
			return "static-thresholds";
		case ModelTest.Percentage:
			return "percentage-thresholds";
		case ModelTest.ZScore:
			return "z-score-thresholds";
		case ModelTest.TTest:
			return "t-test-thresholds";
		case ModelTest.LogNormal:
			return "log-normal-thresholds";
		case ModelTest.Iqr:
			return "iqr-thresholds";
		case ModelTest.DeltaIqr:
			return "delta-iqr-thresholds";
	}
};

const cdfConfig = (modelTest: ModelTest) => {
	return [
		testSelectConfig(modelTest),
		{
			kind: FieldKind.NUMBER,
			label: "Lower Boundary",
			key: "lower_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.cdf_lower_boundary,
		},
		{
			kind: FieldKind.NUMBER,
			label: "Upper Boundary",
			key: "upper_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.cdf_upper_boundary,
		},
		...SAMPLE_SIZE,
	];
};

const iqrConfig = (modelTest: ModelTest) => {
	return [
		testSelectConfig(modelTest),
		{
			kind: FieldKind.NUMBER,
			label: "Lower Boundary",
			key: "lower_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.iqr_lower_boundary,
		},
		{
			kind: FieldKind.NUMBER,
			label: "Upper Boundary",
			key: "upper_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.iqr_upper_boundary,
		},
		...SAMPLE_SIZE,
	];
};

const SAMPLE_SIZE = [
	{
		kind: FieldKind.NUMBER,
		label: "Minimum Sample Size",
		key: "min_sample_size",
		value: "",
		valid: true,
		validate: true,
		nullable: true,
		config: STATISTIC_FIELDS.min_sample_size,
	},
	{
		kind: FieldKind.NUMBER,
		label: "Maximum Sample Size",
		key: "max_sample_size",
		value: "",
		valid: true,
		validate: true,
		nullable: true,
		config: STATISTIC_FIELDS.max_sample_size,
	},
	{
		kind: FieldKind.NUMBER,
		label: "Window Size (seconds)",
		key: "window",
		value: "",
		valid: true,
		validate: true,
		nullable: true,
		config: STATISTIC_FIELDS.window,
	},
];

const FIELDS = {
	[ModelTest.Static]: [
		testSelectConfig(ModelTest.Static),
		{
			kind: FieldKind.NUMBER,
			label: "Lower Boundary",
			key: "lower_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.static_lower_boundary,
		},
		{
			kind: FieldKind.NUMBER,
			label: "Upper Boundary",
			key: "upper_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.static_upper_boundary,
		},
	],
	[ModelTest.Percentage]: [
		testSelectConfig(ModelTest.Percentage),
		{
			kind: FieldKind.NUMBER,
			label: "Lower Boundary",
			key: "lower_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.percentage_lower_boundary,
		},
		{
			kind: FieldKind.NUMBER,
			label: "Upper Boundary",
			key: "upper_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.percentage_upper_boundary,
		},
		...SAMPLE_SIZE,
	],
	[ModelTest.ZScore]: cdfConfig(ModelTest.ZScore),
	[ModelTest.TTest]: cdfConfig(ModelTest.TTest),
	[ModelTest.LogNormal]: cdfConfig(ModelTest.LogNormal),
	[ModelTest.Iqr]: iqrConfig(ModelTest.Iqr),
	[ModelTest.DeltaIqr]: iqrConfig(ModelTest.DeltaIqr),
};

const initForm = (fields: object[]) => {
	const newForm = {};
	for (const field of fields) {
		if (field.key) {
			newForm[field.key] = {
				kind: field.kind,
				label: field.label,
				value: field.value,
				valid: field.valid,
				validate: field.validate,
				nullable: field.nullable,
			};
		}
	}
	return newForm;
};

const Model = (props: Props) => {
	const [test, setTest] = createSignal(ModelTest.TTest);
	const fields = createMemo(() => FIELDS[test()]);

	const [form, setForm] = createStore(initForm(fields()));

	const serializeForm = () => {
		const data: Record<string, undefined | number | string> = {};
		for (const key of Object.keys(form)) {
			const value = form?.[key]?.value;
			switch (form?.[key]?.kind) {
				case FieldKind.SELECT:
					if (form?.[key]?.nullable && !value?.selected) {
						continue;
					}
					data[key] = value?.selected;
					break;
				case FieldKind.NUMBER:
					if (form?.[key]?.nullable && !value) {
						continue;
					}
					data[key] = Number(value);
					break;
				default:
					if (form?.[key]?.nullable && !value) {
						continue;
					}
					if (typeof value === "string") {
						data[key] = value.trim();
					} else {
						data[key] = value;
					}
			}
		}
		return data;
	};

	const handleField = (key: string, value: FieldValue, valid: boolean) => {
		if (key && form?.[key]) {
			if (key === "test") {
				setTest(value?.selected);
				setForm(initForm(FIELDS[value?.selected]));
				props.handleField(serializeForm());
				return;
			}

			if (form?.[key]?.nullable && !value) {
				// biome-ignore lint/style/noParameterAssign: TODO
				value = null;
				// biome-ignore lint/style/noParameterAssign: TODO
				valid = true;
			}

			setForm({
				...form,
				[key]: {
					...form?.[key],
					value,
					valid,
				},
			});

			props.handleField(serializeForm());
		}
	};

	return (
		<>
			<For each={fields()}>
				{(field, _i) => (
					<Field
						kind={field?.kind}
						label={form?.[field?.key]?.label}
						fieldKey={field?.key}
						value={form?.[field?.key]?.value}
						valid={form?.[field?.key]?.valid}
						config={field?.config}
						handleField={handleField}
					/>
				)}
			</For>
		</>
	);
};

export default Model;
