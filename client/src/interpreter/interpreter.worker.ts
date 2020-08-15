export async function init(
  config: any
): Promise<{ interpreter: any; error: any }> {
  try {
    /*eslint no-useless-concat: "off"*/
    const interpreter = await import("interpreter" + "")
    console.log("Interpreter imported")
    const newInterpreter = await interpreter.init("config")
    console.log(newInterpreter)
    return {
      interpreter: newInterpreter,
      error: null,
    }
  } catch (err) {
    console.error(`Unexpected error in loadWasm. [Message: ${err.message}]`)
    return {
      interpreter: null,
      error: err,
    }
  }
}

export async function run(config: any) {
  // TODO Get WASM to actually work
  // can't call alert from web worker
  // interpreter
  return "i am but here"
}
