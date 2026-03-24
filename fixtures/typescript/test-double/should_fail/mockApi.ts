const mockApiClient = {
  get: async (url: string) => ({ data: "mocked" }),
};

const result = await mockApiClient.get("/users");
