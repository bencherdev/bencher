// A dependency graph that contains any wasm must all be imported
// asynchronously. This `bootstrap.js` file does the single async import, so
// that no one else needs to worry about it again.
import("./interpreter.wasm.ts").catch(e =>
  console.error("Error importing `interpreter.wasm.ts`: ", e)
)

// const interpreter = fetch("interpreter.wasm.ts")

// export default interpreter
