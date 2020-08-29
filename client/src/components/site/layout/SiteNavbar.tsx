import React from "react"
import { Navbar, Button } from "react-bulma-components"
import styled from "styled-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faExternalLinkAlt } from "@fortawesome/free-solid-svg-icons"

const BrandName = styled.h1`
  color: black;
  font-size: 300%;
`

const SiteNavbar = (props: { user: any; link: any }) => (
  <Navbar color="white">
    <Navbar.Brand>
      <Navbar.Item renderAs={props?.link} to="/tour">
        <BrandName> TableFlow</BrandName>
      </Navbar.Item>

      <Navbar.Burger
        className="nav-toggle"
        onClick={() => {
          let toggle = document.querySelector(".nav-toggle")
          let menu = document.querySelector(".navbar-menu")
          toggle?.classList.toggle("is-active")
          menu?.classList.toggle("is-active")
        }}
      />
    </Navbar.Brand>
    <Navbar.Menu>
      <Navbar.Container position="start">
        <Navbar.Item renderAs={props?.link} to="/about">
          About
        </Navbar.Item>
        <Navbar.Item renderAs={props?.link} to="/studio">
          Studio
        </Navbar.Item>
        <Navbar.Item renderAs={props?.link} to="/docs">
          Docs
        </Navbar.Item>
        <Navbar.Item renderAs={props?.link} to="/github">
          GitHub
          {"⠀".repeat(1)}
          <span className="icon has-text-primary">
            <FontAwesomeIcon icon={faExternalLinkAlt} />
          </span>
        </Navbar.Item>
      </Navbar.Container>

      <Navbar.Container position="end">
        {props?.user?.isAuth && (
          <Navbar.Item dropdown={true} hoverable={true}>
            <Navbar.Link>
              {props.user?.username.length < 7
                ? props.user?.username +
                  "⠀".repeat(7 - props.user?.username.length)
                : props.user?.username}
            </Navbar.Link>

            <Navbar.Dropdown>
              {props.hasPermission("ADMIN") && (
                <Navbar.Item renderAs={props?.link} to="/admin">
                  Admin
                </Navbar.Item>
              )}
              {props.hasPermission("HUB") && (
                <Navbar.Item renderAs={props?.link} to="/hub">
                  Hub
                </Navbar.Item>
              )}
              {props.hasPermission("SETTINGS") && (
                <Navbar.Item renderAs={props?.link} to="/settings">
                  Settings
                </Navbar.Item>
              )}
              <Navbar.Item renderAs={props?.link} to="/auth/logout">
                <Button outlined={true} fullwidth={true}>
                  Log out
                </Button>
              </Navbar.Item>
            </Navbar.Dropdown>
          </Navbar.Item>
        )}

        {!props?.user?.isAuth && (
          <Navbar.Item renderAs={props?.link} to="/auth/login">
            <Button outlined={true} fullwidth={true}>
              Log in
            </Button>
          </Navbar.Item>
        )}

        {!props?.user?.isAuth && (
          <Navbar.Item renderAs={props?.link} to="/auth/signup">
            <Button color="primary" fullwidth={true}>
              Sign up
            </Button>
          </Navbar.Item>
        )}
      </Navbar.Container>
    </Navbar.Menu>
  </Navbar>
)

export default SiteNavbar
