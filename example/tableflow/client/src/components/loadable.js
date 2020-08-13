import React from "react"
const wasm = import("hello-wasm-pack")

const Wasm = () => {
  wasm
    .then(module => {
      module.main_js()
    })
    .catch(console.error)
  return <p>Here</p>
}

export default Wasm
