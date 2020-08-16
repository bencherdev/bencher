import React from "react"
import styled from "styled-components"

const Link = styled.a`
  color: black;
`

const ExternalLink = (props: any) => {
  return (
    <div>
      {props.black == true ? (
        <Link
          href={props.to}
          rel="noopener noreferrer"
          target="_blank"
          className={props.className}
        >
          {props.children}
        </Link>
      ) : (
        <a
          href={props.to}
          rel="noopener noreferrer"
          target="_blank"
          className={props.className}
        >
          {props.children}
        </a>
      )}
    </div>
  )
}

export default ExternalLink
