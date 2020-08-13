import React from "react"
import { Link } from "gatsby"

import Layout from "../components/layout"
import Image from "../components/image"
import SEO from "../components/seo"

// TODO remove after demoing
// yarn remove @loadable/component
import Loadable from "@loadable/component"
const Wasm = Loadable(() => import("../components/wasm"))
// import Wasm from "../components/wasm"
// import boostrap from "../components/bootstrap"
// import { Helmet } from "react-helmet"
// const wasm = import("hello-wasm-pack")
// const Wasm = Loadable(() => import("../components/loadable"))
// const Wasm = Loadable(() => import("../components/cdn"))

const IndexPage = () => {
  // wasm
  //   .then(module => {
  //     module.greet()
  //   })
  //   .catch(console.error)
  return (
    <Layout>
      <Wasm />
      {/* <Helmet>
        <script src="https://cdn.jsdelivr.net/npm/@bfchen/hello-wasm@0.1.0/hello_wasm.js" />
      </Helmet>
      {window.hellow_wasm} */}
      <SEO title="Home" />
      <h1>Hi people</h1>
      <p>Welcome to your new Gatsby site.</p>
      <p>Now go build something great.</p>
      <div style={{ maxWidth: `300px`, marginBottom: `1.45rem` }}>
        <Image />
      </div>
      <Link to="/page-2/">Go to page 2</Link> <br />
      <Link to="/using-typescript/">Go to "Using TypeScript"</Link>
    </Layout>
  )
}

export default IndexPage
