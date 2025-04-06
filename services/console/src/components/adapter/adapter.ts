import { Adapter } from "../../types/bencher";
import { useSearchParams } from "../../util/url";

const ADAPTER_PARAM = "adapter";
export const BENCHER_ADAPTER_KEY = "BENCHER_ADAPTER";
const CLEAR_ADAPTER = "";

const JSON_ICON = "devicon-json-plain";
const C_SHARP_ICON = "devicon-csharp-line";
const CPP_ICON = "devicon-cplusplus-plain";
const GO_ICON = "devicon-go-original-wordmark";
const JAVA_ICON = "devicon-java-plain";
const JS_ICON = "devicon-javascript-plain";
const PYTHON_ICON = "devicon-python-plain";
const RUBY_ICON = "devicon-ruby-plain";
const RUST_ICON = "devicon-rust-plain";
const SHELL_ICON = "devicon-bash-plain";

export const adapterIcon = (adapter: Adapter) => {
	switch (adapter) {
		case Adapter.Json:
			return JSON_ICON;
		case Adapter.RustBench:
		case Adapter.RustCriterion:
		case Adapter.RustIai:
		case Adapter.RustIaiCallgrind:
			return RUST_ICON;
		case Adapter.CppGoogle:
		case Adapter.CppCatch2:
			return CPP_ICON;
		case Adapter.GoBench:
			return GO_ICON;
		case Adapter.JavaJmh:
			return JAVA_ICON;
		case Adapter.CSharpDotNet:
			return C_SHARP_ICON;
		case Adapter.JsBenchmark:
		case Adapter.JsTime:
			return JS_ICON;
		case Adapter.PythonAsv:
		case Adapter.PythonPytest:
			return PYTHON_ICON;
		case Adapter.RubyBenchmark:
			return RUBY_ICON;
		case Adapter.ShellHyperfine:
			return SHELL_ICON;
		default:
			console.log(`Unsupported adapter: ${adapter}`);
			return;
	}
};

export const getAdapter = () => {
	const adapter = getAdapterInner();
	if (adapter === CLEAR_ADAPTER) {
		clearAdapter();
		return null;
	}

	const [searchParams, setSearchParams] = useSearchParams();
	const adapterParam = searchParams[ADAPTER_PARAM];
	if (validAdapter(adapterParam)) {
		storeAdapterInner(adapterParam as Adapter);
		return adapterParam as Adapter;
	}

	if (validAdapter(adapter)) {
		setSearchParams({
			[ADAPTER_PARAM]: adapter,
		});
		return adapter as Adapter;
	}

	setSearchParams({
		[ADAPTER_PARAM]: null,
	});
	removeAdapterInner();
	return null;
};

export const setAdapter = (adapter: null | Adapter) => {
	const [_searchParams, setSearchParams] = useSearchParams();
	if (validAdapter(adapter)) {
		setSearchParams({
			[ADAPTER_PARAM]: adapter,
		});
		storeAdapterInner(adapter as Adapter);
	} else {
		clearAdapter();
	}
};

export const clearAdapter = () => {
	const [_searchParams, setSearchParams] = useSearchParams();
	setSearchParams({
		[ADAPTER_PARAM]: null,
	});
	storeAdapterInner(CLEAR_ADAPTER);
};

const validAdapter = (adapter: undefined | null | string | Adapter) => {
	switch (adapter) {
		case Adapter.Json:
		case Adapter.RustBench:
		case Adapter.RustCriterion:
		case Adapter.RustIai:
		case Adapter.RustIaiCallgrind:
		case Adapter.CppGoogle:
		case Adapter.CppCatch2:
		case Adapter.GoBench:
		case Adapter.JavaJmh:
		case Adapter.CSharpDotNet:
		case Adapter.JsBenchmark:
		case Adapter.JsTime:
		case Adapter.PythonAsv:
		case Adapter.PythonPytest:
		case Adapter.RubyBenchmark:
		case Adapter.ShellHyperfine:
			return true;
		default:
			return false;
	}
};

const getAdapterInner = () => localStorage.getItem(BENCHER_ADAPTER_KEY);

const storeAdapterInner = (adapter: Adapter | "") =>
	localStorage.setItem(BENCHER_ADAPTER_KEY, adapter);

const removeAdapterInner = () => localStorage.removeItem(BENCHER_ADAPTER_KEY);
