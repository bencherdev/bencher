actor User {}

resource Project {
  permissions = ["view", "create", "edit", "delete", "manage"];
  roles = ["viewer", "developer", "maintainer"];

  "view" if "viewer";
  "create" if "developer";
  "edit" if "developer";
  "delete" if "maintainer";
  "manage" if "maintainer";

  "developer" if "maintainer";
  "viewer" if "developer";
}

# This rule tells Oso how to fetch roles for a project
has_role(actor: User, role_name: String, project: Project) if
  role in actor.roles and
  role_name = role.name and
  project = role.project;

has_permission(_actor: User, "read", project: Project) if
  project.is_public;

allow(actor, action, resource) if
  has_permission(actor, action, resource);