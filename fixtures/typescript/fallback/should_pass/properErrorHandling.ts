try {
  const result = riskyOperation();
} catch (e) {
  console.error("Operation failed:", e);
  throw e;
}
