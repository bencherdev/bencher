import { createEffect, createUniqueId, onMount, ParentProps } from "solid-js";
import { mergeProps } from "solid-js";
import { Link } from "solid-app-router";

function Anchor(props: ParentProps<{ id: string }>) {
  return (
    <a
      class="hover:underline text-solid-dark dark:text-solid-light decoration-solid-lightitem font-bold dark:decoration-solid-darkitem"
      href={`#${props.id}`}
    >
      {props.children}
    </a>
  );
}

const headerBold = "font-bold";

export default {
  strong: (props) => <span class="font-bold">{props.children}</span>,
  h1: (props) => (
    <h1
      {...props}
      class={
        headerBold +
        "heading mt-10 mb-6 -mx-.5 break-words text-4xl leading-tight mdx-heading"
      }
    >
      {props.children}
      <Anchor id={props.id}>{props.children}</Anchor>
    </h1>
  ),
  ssr: (props) => <>{props.children}</>,
  spa: (props) => <></>,
  h2: (props) => {
    return (
      <h2
        {...props}
        class={
          headerBold +
          "heading text-2xl leading-10 my-6 mdx-heading text-solid-accent dark:text-solid-accentlight"
        }
      >
        <Anchor id={props.id}>{props.children}</Anchor>
      </h2>
    );
  },
  h3: (props) => (
    <h3
      {...props}
      class={headerBold + "heading text-2xl leading-9 mt-14 mb-6 mdx-heading"}
    >
      <Anchor id={props.id}>{props.children}</Anchor>
    </h3>
  ),
  h4: (props) => (
    <h4
      {...props}
      class="heading text-xl font-bold leading-9 mt-14 mb-4 mdx-heading"
    >
      <Anchor id={props.id}>{props.children}</Anchor>
    </h4>
  ),
  h5: (props) => (
    <h5 {...props} class="text-xl leading-9 mt-4 mb-4 font-medium mdx-heading">
      <Anchor id={props.id}>{props.children}</Anchor>
    </h5>
  ),
  h6: (props) => (
    <h6 {...props} class="text-xl font-400 mdx-heading">
      <Anchor id={props.id}>{props.children}</Anchor>
    </h6>
  ),
  p: (props) => (
    <p {...props} class="text-lg font-400 my-4">
      {props.children}
    </p>
  ),
  a: (props) => {
    return (
      <Link
        {...props}
        class="dark:text-solid-accentlight break-normal text-solid-accent duration-100 ease-in transition font-semibold leading-normal transition hover:underline"
        target="_blank"
      >
        {props.children}
      </Link>
    );
  },
  li: (props) => (
    <li {...props} class="mb-2">
      {props.children}
    </li>
  ),
  ul: (props) => (
    <ul
      {...props}
      class="list-disc marker:text-solid-accent marker:dark:text-solid-accentlight marker:text-2xl pl-8 mb-2"
    >
      {props.children}
    </ul>
  ),
  ol: (props) => (
    <ol {...props} class="list-decimal pl-8 mb-2">
      {props.children}
    </ol>
  ),
  nav: (props) => <nav {...props}>{props.children}</nav>,
  Link,
  TesterComponent: (props) => (
    <p>
      Remove This Now!!! If you see this it means that markdown custom
      components does work
    </p>
  ),
  code: (props) => {
    return (
      <code className="inline text-code font-mono" {...props}>
        {props.children}
      </code>
    );
  },
  pre: (props) => (
    <>
      {/* <Show when={props.filename?.length > 5}>
        <span {...props} class="h-4 p-1">
          {props.filename}
        </span>
      </Show> */}
      <pre
        {...mergeProps(props, {
          get class() {
            return (
              props.className +
              " " +
              (props.bad ? "border-red-400 border-1" : "")
            );
          },
          get className() {
            return undefined;
          },
        })}
      >
        {props.children}
      </pre>
    </>
  ),
  table: (props) => (
    <table class="w-full max-w-full <sm:portrait:text-xs my-6 rounded-1xl dark:bg-[rgba(17,24,39,1)] shadow-lg text-left overflow-hidden">
      {props.children}
    </table>
  ),
  th: (props) => <th class="p-4 <sm:p-2">{props.children}</th>,
  thead: (props) => (
    <thead class="dark:border-blue-400 border-b-1">{props.children}</thead>
  ),
  td: (props) => <td class="p-4 <sm:p-2">{props.children}</td>,
  tr: (props) => (
    <tr class="dark:even-of-type:bg-[#23406e] light:even-of-type:bg-[#90C2E7]">
      {props.children}
    </tr>
  ),
  "docs-error": (props) => {
    return (
      <div class="docs-error">
        <p>
          <span class="text-red-500">Error:</span>
          {props.children}
        </p>
      </div>
    );
  },
  "docs-info": (props) => {
    return (
      <div class="docs-error">
        <p>
          <span class="text-red-500">Error:</span>
          {props.children}
        </p>
      </div>
    );
  },
  response: (props) => {
    return <span>{props.children}</span>;
  },
  void: (props) => {
    return <span>{props.children}</span>;
  },
  unknown: (props) => {
    return <span>{props.children}</span>;
  },
};
