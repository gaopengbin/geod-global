// Shape checks use the pinned native schema. They do not fill arguments,
// approve plans or replace native value, scope and permission validation.
const guided = new Set(['geod_dataset_resolve','geod_dataset_connect','geod_stac_snapshot','geod_stac_assets','geod_stac_project_plan','geod_stac_download_plan']);
export class ToolArgumentsError extends Error {
  constructor(definition, stopped = false) {
    super('Invalid Agent tool arguments.');
    this.tool = definition.name;
    this.stopped = stopped;
    this.required = [...(definition.inputSchema.required ?? [])];
    this.fields = Object.keys(definition.inputSchema.properties ?? {});
    this.selectionFields = definition.inputSchema.properties?.selections?.items?.required ?? [];
  }
}
export function checkToolArguments(active, definition, args) {
  if (!guided.has(definition?.name)) return;
  const schema = definition.inputSchema;
  const failures = active.argumentFailures ??= new Map();
  if ((failures.get(definition.name) ?? 0) >= 2) throw new ToolArgumentsError(definition, true);
  const object = args && !Array.isArray(args) && typeof args === 'object';
  const invalid = !object || (schema.required ?? []).some(key => !Object.hasOwn(args, key))
    || schema.additionalProperties === false && Object.keys(args).some(key => !Object.hasOwn(schema.properties ?? {}, key));
  if (invalid) {
    failures.set(definition.name, (failures.get(definition.name) ?? 0) + 1);
    throw new ToolArgumentsError(definition);
  }
}
export function toolArgumentGuidance(error) {
  if (!(error instanceof ToolArgumentsError)) return null;
  if (error.stopped) return `Argument correction attempts for ${error.tool} are exhausted in this turn. Stop repeating it and preserve the successful native references. This is not a source or permission failure.`;
  const selection = error.selectionFields.length ? ` Each selections item requires ${error.selectionFields.join(', ')} from an archived eligible asset; do not use connection IDs or assume a dataset layer is an asset key.` : '';
  return `The arguments do not match ${error.tool}. Required fields: ${error.required.join(', ') || 'none'}. Declared fields: ${error.fields.join(', ')}.${selection} Correct the call once using the actual returned references. Do not add undeclared fields or drop required bounds. No approval or execution has occurred.`;
}
