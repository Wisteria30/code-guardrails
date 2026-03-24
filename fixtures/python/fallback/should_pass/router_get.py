# router.get("/path", handler) should NOT trigger
# (this is a web framework routing API, not dict.get with a default)

from fastapi import APIRouter

router = APIRouter()

router.get("/users", get_users)
router.get("/items/{item_id}", get_item)
