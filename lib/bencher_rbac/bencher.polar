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

has_role(user: User, role: String, _server: Server) if
  (user.locked = false and user.admin = true and role = "admin") or
  (user.locked = false and user.admin = false and role = "user") or
  (user.locked = true and role = "locked");

resource Organization {
  permissions = [
    "view",
    "create",
    "edit",
    "delete",
    "manage",
    "view_role",
    "create_role",
    "edit_role",
    "delete_role",
  ];
  roles = ["member", "leader"];

  "view" if "member";
  "view_role" if "member";

  "create" if "leader";
  "edit" if "leader";
  "delete" if "leader";
  "manage" if "leader";
  "create_role" if "leader";
  "edit_role" if "leader";
  "delete_role" if "leader";

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
    user_role matches [org.id, role]
  );

resource Project {
  permissions = [
    "view",
    "create",
    "edit",
    "delete",
    "manage",
    "view_role",
    "create_role",
    "edit_role",
    "delete_role",
  ];
  roles = ["viewer", "developer", "maintainer"];
  relations = { owner: Organization };

  "view" if "viewer";
  "view_role" if "viewer";

  "create" if "developer";
  "edit" if "developer";
  "delete" if "developer";

  "manage" if "maintainer";
  "create_role" if "maintainer";
  "edit_role" if "maintainer";
  "delete_role" if "maintainer";

  "developer" if "maintainer";
  "viewer" if "developer";
}

has_relation(org: Organization, "owner", project: Project) if
  org.id = project.organization_id;

has_role(user: User, role: String, project: Project) if
  (
    server := new Server() and
    has_role(user, "admin", server)
  )
  or
  (
    org := new Organization(project.organization_id) and
    has_role(user, "leader", org)
  )
  or
  (
    user_role in user.projects and
    user_role matches [project.id, role]
  );
