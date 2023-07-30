import bencher_valid_init, { is_valid_jwt } from "bencher_valid";


const WasmButton = () => <button onClick={(e) => bencher_valid_init().then(() => console.log(is_valid_jwt("bad")))}>Validate JWT</button>;

export default WasmButton;