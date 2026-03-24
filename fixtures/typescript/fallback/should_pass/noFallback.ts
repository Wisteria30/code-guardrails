const name = getUserName();
const config = loadConfig();

if (name && config) {
  process(name, config);
}
