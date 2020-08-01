import React from "react"
import { Link } from "gatsby"

import SitePage from "../components/site/pages/sitepage"
import AboutPage from "../components/site/pages/aboutpage"

const About = () => (
  <SitePage link={Link}>
    <AboutPage />
  </SitePage>
)

export default About
