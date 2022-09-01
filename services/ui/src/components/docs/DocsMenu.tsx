import { Link } from "solid-app-router";
import slugify from "slugify";

const asSlug = (text: string) => {
  return slugify(text, {
    lower: true,
  });
};

export enum Section {
  HOW_TO = "How To",
  REFERENCE = "Reference",
}

export enum HowTo {
  QUICK_START = "Quick Start",
}

export enum Reference {
  API = "API",
}

const DocsMenu = (props) => {
  const getDocsPath = (section: Section, page: string) => {
    return `/docs/${asSlug(section)}/${asSlug(page)}`;
  };

  return (
    <aside class="menu">
      <p class="menu-label">{Section.HOW_TO}</p>
      <ul class="menu-list">
        <li>
          <Link href={getDocsPath(Section.HOW_TO, HowTo.QUICK_START)}>
            {HowTo.QUICK_START}
          </Link>
        </li>
      </ul>
      <p class="menu-label">{Section.REFERENCE}</p>
      <ul class="menu-list">
        <Link href={getDocsPath(Section.REFERENCE, Reference.API)}>
          {Reference.API}
        </Link>
      </ul>
    </aside>
  );
};

export default DocsMenu;
