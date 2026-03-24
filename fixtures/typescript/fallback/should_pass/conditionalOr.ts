// || in conditions should NOT trigger (assignment-only rule)
if (!COGNITO_DOMAIN || !COGNITO_CLIENT_ID) {
  throw new Error("Missing config");
}

while (pending || retrying) {
  await sleep(100);
}
