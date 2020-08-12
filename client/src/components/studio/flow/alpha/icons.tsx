import React from "react"

export const Circle = () => <circle r={10} cx={10} fill="lightblue" />

export const Triangle = () => <polygon points="10,0 20,15 0,15" fill="cyan" />

export const Rectangle = () => (
  <rect width={15} height={15} x={3} fill="steelblue" />
)
