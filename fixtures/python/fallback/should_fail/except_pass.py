try:
    result = risky_operation()
except Exception:
    pass

try:
    connect()
except ConnectionError as e:
    pass
