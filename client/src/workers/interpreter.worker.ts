import interpreter from "../wasm/interpreter.bootstrap"

export async function run(config: any) {
  // TODO Get WASM to actually work
  // can't call alert from web worker
  interpreter
  return "i am but here"
}
