import { render } from "solid-js/web";
import { CounterProvider, useCounter } from "./counter-store";

const MiddleComponent = () => <NestedComponent />;

const NestedComponent = () => {
  const [count, { increment, decrement }] = useCounter();
  return (
    <>
      <p>{count()}</p>
      <button onClick={increment}>+</button>
      <button onClick={decrement}>-</button>
    </>
  );
};

const App = () => (
  <CounterProvider count={7}>
    <MiddleComponent />
  </CounterProvider>
);

render(App, document.body);
