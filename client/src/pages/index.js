import React from "react"
import { Link } from "gatsby"

import "react-bulma-components/dist/react-bulma-components.min.css"
import "../styles/_variables.sass"
import { Button } from "react-bulma-components"

import Layout from "../components/layout"
import Image from "../components/image"
import SEO from "../components/seo"

const IndexPage = () => (
  <Layout>
    <SEO title="Home" />
    <h1>Hi people</h1>
    <p>Welcome to your new Gatsby site.</p>
    <p>Now go build something great.</p>
    <div style={{ maxWidth: `300px`, marginBottom: `1.45rem` }}>
      <Image />
    </div>
    <Link to="/studio/">Studio</Link> <br />
    <Link to="/client/">Client Side</Link> <br />
    <Link to="/page-2/">Go to page 2</Link> <br />
    <Link to="/using-typescript/">Go to "Using TypeScript"</Link> <br />
    <Link to="/terms/">Terms of Use</Link> <br />
    <Button color="primary">Bulma button</Button>
  </Layout>
)

export default IndexPage
