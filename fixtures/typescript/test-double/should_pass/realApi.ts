class ApiClient {
  constructor(private baseUrl: string) {}

  async get(path: string): Promise<Response> {
    return fetch(`${this.baseUrl}${path}`);
  }
}
