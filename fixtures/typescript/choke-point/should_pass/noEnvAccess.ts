// Clean application code
interface Config {
  port: number;
  host: string;
}

function createServer(config: Config) {
  return { port: config.port };
}
