import memberFieldsConfig from "../../fields/config/org/memberFieldsConfig";
import { isAllowedOrganization, OrganizationPermission } from "../../site/util";
import { Button, Card, Display, Field, Operation, PerfTab, Row } from "./types";
import { BENCHER_API_URL, parentPath, invitePath, viewSlugPath } from "./util";

const ROLE_VALUE = {
  selected: "leader",
  options: [
    // TODO Team Management
    // {
    //   value: "member",
    //   option: "Member",
    // },
    {
      value: "leader",
      option: "Leader",
    },
  ],
};

const MembersConfig = {
  [Operation.LIST]: {
    operation: Operation.LIST,
    header: {
      title: "Organization Members",
      buttons: [
        {
          kind: Button.INVITE,
          path: invitePath,
          is_allowed: (path_params) =>
            isAllowedOrganization(
              path_params,
              OrganizationPermission.CREATE_ROLE
            ),
        },
        { kind: Button.REFRESH },
      ],
    },
    table: {
      url: (path_params) =>
        `${BENCHER_API_URL}/v0/organizations/${path_params?.organization_slug}/members`,
      add: {
        path: invitePath,
        text: "Invite an Organization Member",
      },
      row: {
        key: "name",
        items: [
          {
            kind: Row.TEXT,
            key: "slug",
          },
          {
            kind: Row.TEXT,
            key: "email",
          },
          {
            kind: Row.SELECT,
            key: "role",
            value: ROLE_VALUE,
          },
          {},
        ],
        button: {
          text: "View",
          path: (pathname, datum) => viewSlugPath(pathname, datum),
        },
      },
    },
  },
  [Operation.ADD]: {
    operation: Operation.ADD,
    header: {
      title: "Invite to Organization",
      path: parentPath,
    },
    form: {
      url: (path_params) =>
        `${BENCHER_API_URL}/v0/organizations/${path_params.organization_slug}/members`,
      fields: [
        {
          kind: Field.HIDDEN,
          key: "organization",
          path_param: "organization_slug",
        },
        {
          kind: Field.INPUT,
          label: "Name",
          key: "name",
          value: "",
          valid: null,
          validate: true,
          nullify: false,
          clear: false,
          config: memberFieldsConfig.name,
        },
        {
          kind: Field.INPUT,
          label: "Email",
          key: "email",
          value: "",
          valid: null,
          validate: true,
          nullify: false,
          clear: false,
          config: memberFieldsConfig.email,
        },
        {
          kind: Field.SELECT,
          label: "Role",
          key: "role",
          value: ROLE_VALUE,
          validate: false,
          nullify: false,
          clear: false,
          config: memberFieldsConfig.role,
        },
      ],
      path: parentPath,
    },
  },
  [Operation.VIEW]: {
    operation: Operation.VIEW,
    header: {
      key: "name",
      path: parentPath,
    },
    deck: {
      url: (path_params) =>
        `${BENCHER_API_URL}/v0/organizations/${path_params?.organization_slug}/members/${path_params?.member_slug}`,
      cards: [
        {
          kind: Card.FIELD,
          label: "Member Name",
          key: "name",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Member Slug",
          key: "slug",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Member UUID",
          key: "uuid",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Member Email",
          key: "email",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Role",
          key: "role",
          display: Display.SELECT,
          is_allowed: (path_params) =>
            isAllowedOrganization(
              path_params,
              OrganizationPermission.EDIT_ROLE
            ),
          field: {
            kind: Field.SELECT,
            key: "role",
            value: ROLE_VALUE,
            validate: false,
            nullify: false,
            clear: false,
            config: memberFieldsConfig.role,
          },
        },
      ],
      buttons: false,
    },
  },
};

export default MembersConfig;
