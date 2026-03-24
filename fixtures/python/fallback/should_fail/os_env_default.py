import os

db_url = os.getenv("DATABASE_URL", "sqlite:///dev.db")
port = os.environ.get("PORT", "8080")
