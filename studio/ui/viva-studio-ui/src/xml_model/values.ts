// Draft values live outside the UiGraph contract so we can evolve editor UX
// without mutating the read-only structural model.
export type NodeValue = number | string | boolean | { enumName: string } | null;

export type ValueError = { message: string };
