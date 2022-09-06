actor User {}

resource Server {
  permissions = ["session", "administer"];
  roles = ["locked", "user", "admin"];

  "session" if "user";
  "administer" if "admin";

  "user" if "admin";
}

resource Org {
  permissions = [
    "read",
    "create_projects",
    "list_projects",
    "create_role_assignments",
    "list_role_assignments",
    "update_role_assignments",
    "delete_role_assignments",
  ];
  roles = ["member", "leader"];
  relations = { host: Server };

  "read" if "member";
  "list_projects" if "member";
  "list_role_assignments" if "member";

  "create_projects" if "leader";
  "create_role_assignments" if "leader";
  "update_role_assignments" if "leader";
  "delete_role_assignments" if "leader";

  "member" if "leader";

  "leader" if "admin" on "host";
}

has_relation(_server: Server, "host", _org: Org);

resource Project {
  permissions = ["view", "create", "edit", "delete", "manage"];
  roles = ["viewer", "developer", "owner"];
  relations = { host: Server, parent: Org };

  "view" if "viewer";
  "create" if "developer";
  "edit" if "developer";
  "delete" if "owner";
  "manage" if "owner";

  "developer" if "owner";
  "viewer" if "developer";

  "viewer" if "member" on "parent";
  "owner" if "leader" on "parent";

  "owner" if "admin" on "host";
}

has_relation(_server: Server, "host", _project: Project);

# This rule tells Oso how to fetch roles for a project
has_role(user: User, role_name: String, project: Project) if
  role in user.roles and
  role.name = role_name and
  role.project = project;

allow(actor, action, resource) if
  has_permission(actor, action, resource);