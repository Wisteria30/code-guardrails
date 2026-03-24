// ?? in conditions/returns should NOT trigger (assignment-only rule)
if (value ?? alternative) {
  process();
}

function getOrThrow(val: string | null): string {
  return val ?? throwError("missing");
}
