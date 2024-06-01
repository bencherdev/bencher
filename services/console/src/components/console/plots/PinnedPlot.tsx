import type { Params } from "astro";
import Pinned from "./Pinned";
import { authUser, isAllowedProjectManage } from "../../../util/auth";
import { createMemo, createResource } from "solid-js";
import { httpGet } from "../../../util/http";
import type { JsonPlot } from "../../../types/bencher";

export interface Props {
	apiUrl: string;
	params: Params;
}

const PinnedPlot = (props: Props) => {
	const user = authUser();

	const project_slug = createMemo(() => props.params?.project);
	const plotFetcher = createMemo(() => {
		return {
			token: user?.token,
		};
	});
	const getPlot = async (fetcher: {
		token: string;
	}) => {
		const path = `/v0/projects/${props.params?.project}/plots/${props.params?.plot}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				return resp?.data as JsonPlot;
			})
			.catch((error) => {
				console.error(error);
				return;
			});
	};
	const [plot] = createResource<JsonPlot>(plotFetcher, getPlot);

	const allowedFetcher = createMemo(() => {
		return {
			apiUrl: props.apiUrl,
			params: props.params,
		};
	});
	const getAllowed = async (fetcher: {
		apiUrl: string;
		params: Params;
	}) => {
		return await isAllowedProjectManage(fetcher.apiUrl, fetcher.params);
	};
	const [isAllowed] = createResource(allowedFetcher, getAllowed);

	return (
		<Pinned
			apiUrl={props.apiUrl}
			params={props.params}
			user={user}
			project_slug={project_slug}
			isAllowed={isAllowed}
			plot={plot()}
			index={() => 0}
			total={() => 1}
			movePlot={() => {}}
			updatePlot={() => {}}
			removePlot={() => {}}
			search={() => props.params?.plot}
		/>
	);
};

export default PinnedPlot;
