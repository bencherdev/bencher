import memberFieldsConfig from "../../fields/config/org/memberFieldsConfig";
import { isAllowedOrganization } from "../../site/util";
import { Button, Card, Field, Operation, PerfTab, Row } from "./types";
import { BENCHER_API_URL, parentPath, invitePath, viewSlugPath } from "./util";

const MembersConfig = {
  [Operation.LIST]: {
    operation: Operation.LIST,
    header: {
      title: "Organization Members",
      buttons: [
        {
          kind: Button.INVITE,
          path: (pathname) => {
            return invitePath(pathname);
          },
          is_allowed: isAllowedOrganization
        },
        { kind: Button.REFRESH },
      ],
    },
    table: {
      url: (path_params) => {
        return `${BENCHER_API_URL}/v0/organizations/${path_params?.organization_slug}/members`;
      },
      add: {
        path: (pathname) => {
          return invitePath(pathname);
        },
        text: "Invite an Organization Member",
      },
      row: {
        key: "name",
        items: [
          {
            kind: Row.TEXT,
            key: "slug",
          },
          {},
          {
            kind: Row.TEXT,
            key: "email",
          },
          {},
        ],
        button: {
          text: "View",
          path: (pathname, datum) => {
            return viewSlugPath(pathname, datum);
          },
        },
      },
    },
  },
  [Operation.INVITE]: {
    operation: Operation.INVITE,
    header: {
      title: "Invite to Organization",
      path: (pathname) => {
        return parentPath(pathname);
      },
    },
    form: {
      url: `${BENCHER_API_URL}/v0/invites`,
      fields: [
        {
          kind: Field.INPUT,
          key: "name",
          label: true,
          value: "",
          valid: null,
          validate: true,
          nullify: false,
          clear: false,
          config: memberFieldsConfig.name,
        },
        {
          kind: Field.INPUT,
          key: "email",
          label: true,
          value: "",
          valid: null,
          validate: true,
          nullify: false,
          clear: false,
          config: memberFieldsConfig.email,
        },
        {
          kind: Field.INPUT,
          key: "organization",
          label: true,
          value: "",
          valid: null,
          validate: false,
          nullify: true,
          clear: false,
          config: memberFieldsConfig.organization,
        },
        {
          kind: Field.INPUT,
          key: "role",
          label: true,
          value: "",
          validate: false,
          nullify: false,
          clear: false,
          config: memberFieldsConfig.role,
        },
      ],
      path: (pathname) => {
        return parentPath(pathname);
      },
    },
  },
  [Operation.VIEW]: {
    operation: Operation.VIEW,
    header: {
      key: "name",
      path: (pathname) => {
        return parentPath(pathname);
      },
    },
    deck: {
      url: (path_params) => {
        return `${BENCHER_API_URL}/v0/organizations/${path_params?.organization_slug}/members/${path_params?.member_slug}`;
      },
      cards: [
        {
          kind: Card.FIELD,
          field: "Member Name",
          key: "name",
        },
        {
          kind: Card.FIELD,
          field: "Member Slug",
          key: "slug",
        },
        {
          kind: Card.FIELD,
          field: "Member Email",
          key: "email",
        },
        {
          kind: Card.FIELD,
          field: "Role",
          key: "role",
        },
      ],
      buttons: false,
    },
  },
};

export default MembersConfig;
