import { createSignal, Show } from "solid-js";
import { getAdapter } from "./adapter";

const SelectedAdapter = () => {
    const [adapter, setAdapter] = createSignal(getAdapter());

    setInterval(() => {
        const new_adapter = getAdapter();
        if (new_adapter && new_adapter !== adapter()) {
            setAdapter(new_adapter);
        }
    }, 100);

    return <Show when={adapter()}>
        <div>{adapter()}</div>
    </Show>;
};

export default SelectedAdapter;
