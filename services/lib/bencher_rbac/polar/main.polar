actor User {}

resource Server {
  permissions = ["session", "administer"];
  roles = ["locked", "user", "admin"];

  "session" if "user";
  "administer" if "admin";

  "user" if "admin";
}

resource Project {
  permissions = ["view", "create", "edit", "delete", "manage"];
  roles = ["viewer", "developer", "maintainer"];
  relations = { host: Server };

  "view" if "viewer";
  "create" if "developer";
  "edit" if "developer";
  "delete" if "maintainer";
  "manage" if "maintainer";

  "developer" if "maintainer";
  "viewer" if "developer";

  "maintainer" if "admin" on "host";
}

has_relation(server: Server, "host", project: Project) if
  server = project.server;

# This rule tells Oso how to fetch roles for a project
has_role(user: User, role_name: String, project: Project) if
  role in user.roles and
  role.name = role_name and
  role.project = project;

has_permission(_actor: User, "read", project: Project) if
  project.is_public;

allow(actor, action, resource) if
  has_permission(actor, action, resource);