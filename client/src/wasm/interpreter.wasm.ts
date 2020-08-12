// import * as interpreter from "interpreter"

// interpreter.greet()

import("interpreter")
  .then(module => {
    module.greet()
  })
  .catch(e => console.error("Error wasm: ", e))
