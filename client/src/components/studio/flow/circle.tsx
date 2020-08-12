import React from "react"

const Circle = React.forwardRef((props, ref) => (
  <circle fill={"black"} r={50} ref={ref} />
))

export default Circle
