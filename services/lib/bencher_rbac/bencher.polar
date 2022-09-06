allow(actor, action, resource) if
  has_permission(actor, action, resource);

actor User {}

resource Server {
  permissions = ["session", "administer"];
  roles = ["locked", "user", "admin"];

  "session" if "user";
  "administer" if "admin";

  "user" if "admin";
}

# This rule tells Oso how to fetch roles for a server
has_role(user: User, role: String, _server: Server) if
  (user.locked = false and user.admin = true and role = "admin") or
  (user.locked = false and user.admin = false and role = "user") or
  (user.locked = true and role = "locked");

resource Organization {
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

  "read" if "member";
  "list_projects" if "member";
  "list_role_assignments" if "member";

  "create_projects" if "leader";
  "create_role_assignments" if "leader";
  "update_role_assignments" if "leader";
  "delete_role_assignments" if "leader";

  "member" if "leader";
}

has_role(user: User, role: String, org: Organization) if
  (
    server := new Server() and
    has_role(user, "admin", server)
  )
  or
  (
    user_role in user.organizations and
    user_role matches [org.uuid, role]
  );


resource Project {
  permissions = ["view", "create", "edit", "delete", "manage"];
  roles = ["viewer", "developer", "maintainer"];
  relations = { parent: Organization };

  "view" if "viewer";
  "create" if "developer";
  "edit" if "developer";
  "delete" if "developer";
  "manage" if "maintainer";

  "developer" if "maintainer";
  "viewer" if "developer";
}

has_relation(org: Organization, "parent", project: Project) if
  org.uuid = project.parent;


has_role(user: User, role: String, project: Project) if
  (
    server := new Server() and
    has_role(user, "admin", server)
  )
  or
  (
    org := new Organization(project.parent) and
    has_role(user, "leader", org)
  )
  or
  (
    user_role in user.projects and
    user_role matches [project.uuid, role]
  );
