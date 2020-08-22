import React from "react"
import { navigate } from "gatsby"
import { Menu } from "react-bulma-components"

import AddButton from "./AddButton"
import studioMenu from "./config/studioMenu"

const DashboardMenu = (props: { path: string }) => {
  const menu = studioMenu(props.path)

  function handleMenuClick(event: Event, path: string) {
    event.preventDefault()
    navigate(path)
  }

  function handleSections() {
    return menu.map((section, sectionIndex) => {
      return (
        <Menu.List key={sectionIndex} title={section.title}>
          {section.tabs.map((tab, tabIndex) => {
            if (tab.label) {
              return (
                <React.Fragment key={tabIndex}>
                  <Menu.List.Item
                    active={tab.path === window.location.pathname && true}
                    onClick={(event: Event) => handleMenuClick(event, tab.path)}
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
      <AddButton />
      {handleSections()}
    </Menu>
  )
}

export default DashboardMenu
