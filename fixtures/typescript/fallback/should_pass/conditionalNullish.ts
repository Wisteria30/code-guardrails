// ?? in conditions/returns should NOT trigger (assignment-only rule)
if (value ?? fallback) {
  process();
}

function getOrThrow(val: string | null): string {
  return val ?? throwError("missing");
}
