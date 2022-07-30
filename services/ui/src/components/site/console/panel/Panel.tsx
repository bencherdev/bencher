export interface Panel {
  section: string;
  operation: string;
  slug: string | null;
}

export const Section = {
  PROJECTS: "projects",
};

export const Operation = {
  ADD: "add",
  LIST: "list",
  VIEW: "view",
  EDIT: "edit",
  DELETE: "delete",
};
