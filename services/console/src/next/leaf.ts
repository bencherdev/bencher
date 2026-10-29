import { tailOf } from "@bencherdev/ui/tail";

/** A name by its last `::` or `/` segment, which tells siblings apart. */
export const leafOf = (name: string): string => tailOf(name)[1] || name;
