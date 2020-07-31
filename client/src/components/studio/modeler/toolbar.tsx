import React from "react"
import { Link } from "@reach/router"
import { Navbar, Button } from "react-bulma-components"
import ExternalLink from "../../utils/externallink"

const Toolbar = (props: any) => (
  <Navbar fixed="top" color="white">
    <Navbar.Menu>
      <Navbar.Container position="start">
        <Navbar.Item>Tool</Navbar.Item>
      </Navbar.Container>

      <Navbar.Container position="end">
        <Navbar.Item dropdown={true} hoverable={true}>
          <Navbar.Dropdown>
            <Navbar.Item>Tool A</Navbar.Item>
            <Navbar.Item>Tool B</Navbar.Item>
            <Navbar.Item>Tool C</Navbar.Item>
          </Navbar.Dropdown>
        </Navbar.Item>
      </Navbar.Container>
    </Navbar.Menu>
  </Navbar>
)

export default Toolbar
