import os

# This is application code, not boundary/settings
timeout = os.getenv("TIMEOUT", "30")
host = os.environ.get("HOST", "localhost")
port = os.environ["PORT"]
