class MockPaymentGateway:
    def charge(self, amount):
        return {"status": "ok"}

mock_client = MockPaymentGateway()
result = mock_client.charge(100)
