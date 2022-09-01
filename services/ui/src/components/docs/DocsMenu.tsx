import { Link } from "solid-app-router";
import slugify from "slugify";

const asSlug = (text: string) => {
  return slugify(text, {
    lower: true,
  });
};

export enum Section {
  HOW_TO = "How To",
}

export enum HowTo {
  QUICK_START = "Quick Start",
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
      <p class="menu-label">User</p>
      <ul class="menu-list">
        <li>
          <Link href="/console/user/account">Account</Link>
        </li>
        <li>
          <Link href="/console/user/settings">Settings</Link>
        </li>
      </ul>
    </aside>
  );
};

export default DocsMenu;
