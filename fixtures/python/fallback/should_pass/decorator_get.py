# router.get() and similar method calls should NOT trigger
# (they are not dict.get with a default value)

class Router:
    def get(self, path, handler):
        pass

router = Router()


@router.get("/health")
def health():
    return {"status": "ok"}
