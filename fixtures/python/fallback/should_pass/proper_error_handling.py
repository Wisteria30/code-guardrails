try:
    result = risky_operation()
except ValueError as e:
    logger.error("Operation failed: %s", e)
    raise
