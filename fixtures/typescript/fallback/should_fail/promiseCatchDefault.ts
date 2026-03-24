const data = fetchApi().catch((err) => []);
const data2 = fetchApi().catch(() => null);
