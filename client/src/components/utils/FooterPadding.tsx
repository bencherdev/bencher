import React from "react"

const FooterPadding = (props: { margin: undefined | number }) => (
  <div style={{ margin: `0 0 ${props.margin ? props.margin : 1000}px` }} />
)

export default FooterPadding
