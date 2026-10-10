import {
	type DefaultError,
	type QueryKey,
	QueryObserver,
	type QueryObserverOptions,
	type QueryObserverResult,
	useQueryClient,
} from "@tanstack/solid-query";
import { createComputed, createSignal, on, onCleanup } from "solid-js";

/**
 * A query's result as the cache holds it. solid-query's `useQuery` reads
 * `data` through a resource, and the Suspense around a page then falls back
 * on every later write to that query: the page is taken out and put back, its
 * focus lost and an open dialog no longer modal. This reads no resource.
 */
export const useQueryResult = <TData>(
	options: () => QueryObserverOptions<
		TData,
		DefaultError,
		TData,
		TData,
		QueryKey
	>,
) => {
	const client = useQueryClient();
	const defaulted = () => client.defaultQueryOptions(options());
	const observer = new QueryObserver(client, defaulted());
	const [result, setResult] = createSignal<
		QueryObserverResult<TData, DefaultError>
	>(observer.getOptimisticResult(defaulted()), { equals: false });
	createComputed(
		on(
			defaulted,
			(next) => {
				observer.setOptions(next);
				setResult(observer.getOptimisticResult(next));
			},
			{ defer: true },
		),
	);
	onCleanup(observer.subscribe(setResult));
	return result;
};
