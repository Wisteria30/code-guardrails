class PaymentGateway:
    def __init__(self, api_key: str):
        self.api_key = api_key

    def charge(self, amount: int) -> dict:
        return self._call_api("/charge", {"amount": amount})

    def _call_api(self, path: str, data: dict) -> dict:
        raise NotImplementedError
