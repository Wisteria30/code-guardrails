function getData() {
  try {
    return fetchData();
  } catch (e) {
    return [];
  }
}
