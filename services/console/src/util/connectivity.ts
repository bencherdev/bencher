import { createRoot, createSignal } from "solid-js";

export enum ApiState {
	CONNECTED = "connected",
	RECONNECTING = "reconnecting",
	DISCONNECTED = "disconnected",
}

const connectivity = createRoot(() => {
	const [apiState, setApiState] = createSignal<ApiState>(ApiState.CONNECTED);
	return { apiState, setApiState };
});

export const apiState = connectivity.apiState;
export const setApiState = connectivity.setApiState;
