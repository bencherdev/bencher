import React from "react"
import { Helmet } from "react-helmet"

const Wasm = () => {
  return (
    <React.Fragment>
      <Helmet>
        <script
          type="module"
          src="https://cdn.jsdelivr.net/npm/@bfchen/hello-wasm@0.1.0/hello_wasm.js"
        />
      </Helmet>
      {window?.hellow_wasm?.greet()}
      <p>Here</p>
    </React.Fragment>
  )
}

export default Wasm
