import { Link } from "solid-app-router";
import slugify from "slugify";

enum Section {
  HOW_TO = "How To",
  REFERENCE = "Reference",
}

enum Page {
  QUICK_START = "Quick Start",
  API = "API",
  CHANGELOG = "Changelog",
}

const asSlug = (text: string) => {
  return slugify(text, {
    lower: true,
  });
};

const getDocsPath = (section: Section, page: string) => {
  return `/docs/${asSlug(section)}/${asSlug(page)}`;
};

const DocsMenu = (props) => {
  return (
    <aside class="menu">
      <p class="menu-label">{Section.HOW_TO}</p>
      <ul class="menu-list">
        <li>
          <Link href={getDocsPath(Section.HOW_TO, Page.QUICK_START)}>
            {Page.QUICK_START}
          </Link>
        </li>
      </ul>
      <p class="menu-label">{Section.REFERENCE}</p>
      <ul class="menu-list">
        <Link href={getDocsPath(Section.REFERENCE, Page.API)}>{Page.API}</Link>
        <Link href={getDocsPath(Section.REFERENCE, Page.CHANGELOG)}>
          {Page.CHANGELOG}
        </Link>
      </ul>
    </aside>
  );
};

export default DocsMenu;
