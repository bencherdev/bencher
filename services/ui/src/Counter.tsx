import { createSignal, createEffect } from "solid-js";

const [count, setCount] = createSignal(2);

export function Multiplier(prop) {
  return (
    <div>
      <h1>
        {count()} * {prop.by} = {count() * prop.by}
      </h1>
      <button onClick={() => setCount(count() + 1)}>Counter</button>
    </div>
  );
}
