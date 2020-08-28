import React, { useState } from "react"
import { navigate } from "gatsby"
import { Menu } from "react-bulma-components"

import NewButton from "./button/NewButton"
import studioMenu from "./config/studioMenu"

const DashboardMenu = (props: { path: string }) => {
  const menu = studioMenu(props.path)
  const [active, setActive] = useState("")

  function handleMenuClick(event: Event, path: string) {
    event.preventDefault()
    navigate(path)
  }

  function handleSections() {
    return menu.map((section, sectionIndex) => {
      return (
        <Menu.List key={sectionIndex} title={section.title}>
          {section.tabs.map((tab, tabIndex) => {
            if (
              (tab?.path === window.location.pathname ||
                tab?.path + "/" === window.location.pathname) &&
              tab?.active &&
              tab.active !== active
            ) {
              setActive(tab.active)
            }
            if (tab?.label && tab?.label !== "") {
              return (
                <React.Fragment key={tabIndex}>
                  <Menu.List.Item
                    active={tab?.path === active}
                    onClick={(event: Event) =>
                      handleMenuClick(event, tab?.path)
                    }
                  >
                    {tab.label}
                  </Menu.List.Item>
                </React.Fragment>
              )
            }
            return null
          })}
        </Menu.List>
      )
    })
  }

  return (
    <Menu>
      <NewButton />
      {handleSections()}
    </Menu>
  )
}

export default DashboardMenu
