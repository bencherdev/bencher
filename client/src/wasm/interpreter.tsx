import React, { useState } from "react"
// const wasm = import("../../../pkg/index.js")
// const wasm = require("../../../pkg/index.js")

const Wasm = () => {
  const [wasmModule, setWasmModule] = useState()

  const loadWasm = async () => {
    try {
      //   if (typeof window === "undefined") {
      //     return
      //   }
      /*eslint no-useless-concat: "off"*/
      const wasm = await import("interpreter" + "")
      setWasmModule({ wasm })
      console.log("wasm set")
    } catch (err) {
      console.error(`Unexpected error in loadWasm. [Message: ${err.message}]`)
    }
  }

  const callWasm = async ({ wasm }: any) => {
    console.log("calling wasm")
    const res = await wasm.greet()
    console.log(res)
  }

  // load wasm asynchronously
  wasmModule === undefined && loadWasm()

  if (wasmModule !== undefined) {
    callWasm(wasmModule)
  }

  //   wasm
  //     .then(module => {
  //       module.main_js()
  //     })
  //     .catch(console.error)
  // wasm.main_js()
  return <p>Here</p>
}

export default Wasm
