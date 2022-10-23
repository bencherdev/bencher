import memberFieldsConfig from "../../fields/config/org/memberFieldsConfig";
import { isAllowedOrganization, OrganizationPermission } from "../../site/util";
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
          is_allowed: (path_params) => isAllowedOrganization(path_params, OrganizationPermission.CREATE_ROLE),
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
          kind: Field.HIDDEN,
          key: "organization",
          path_param: "organization_slug",
        },
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
          kind: Field.SELECT,
          key: "role",
          label: true,
          value: {
            selected: "member",
            options: [
              {
                value: "member",
                option: "Member",
              },
              {
                value: "leader",
                option: "Leader",
              }
            ],
          },
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
          label: "Member Name",
          key: "name",
        },
        {
          kind: Card.FIELD,
          label: "Member Slug",
          key: "slug",
        },
        {
          kind: Card.FIELD,
          label: "Member Email",
          key: "email",
        },
        {
          kind: Card.FIELD,
          label: "Role",
          key: "role",
          is_allowed: (path_params) => isAllowedOrganization(path_params, OrganizationPermission.EDIT_ROLE),
        },
      ],
      buttons: false,
    },
  },
};

export default MembersConfig;
